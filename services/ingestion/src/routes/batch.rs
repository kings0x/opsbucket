use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use opsbucket_shared::events::{AnyEvent, BatchPayload, RawEvent};
use serde::Deserialize;
use serde_json::json;

use crate::validation::{validate_batch, ValidationError};
use crate::AppState;

#[derive(Deserialize)]
pub struct BatchQuery {
    write_key: Option<String>,
}

fn extract_write_key(headers: &HeaderMap, query: &BatchQuery) -> Option<String> {
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(key) = auth.strip_prefix("Bearer ") {
            return Some(key.to_string());
        }
    }
    query.write_key.clone()
}

fn extract_ip(headers: &HeaderMap) -> String {
    if let Some(fwd) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(ip) = fwd.split(',').next().map(|s| s.trim()) {
            if !ip.is_empty() {
                return ip.to_string();
            }
        }
    }
    if let Some(real_ip) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        if !real_ip.is_empty() {
            return real_ip.to_string();
        }
    }
    "0.0.0.0".to_string()
}

fn stamp_events(events: Vec<AnyEvent>, project_id: &str, sent_at: &str, ip: &str) -> Vec<RawEvent> {
    let received_at = Utc::now().to_rfc3339();
    events
        .into_iter()
        .map(|e| {
            RawEvent::from_any(
                e,
                project_id.to_string(),
                received_at.clone(),
                sent_at.to_string(),
                ip.to_string(),
            )
        })
        .collect()
}

fn json_response(status: StatusCode, value: serde_json::Value) -> Response {
    (status, Json(value)).into_response()
}

pub async fn post_batch(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<BatchQuery>,
    bytes: Bytes,
) -> Response {
    let write_key_str = match extract_write_key(&headers, &query) {
        Some(k) => k,
        None => {
            return json_response(
                StatusCode::UNAUTHORIZED,
                json!({"error": "invalid_write_key"}),
            );
        }
    };

    let rate_result = {
        let mut limiter = state.rate_limiter.lock().unwrap();
        limiter.check(&write_key_str)
    };

    if !rate_result.allowed {
        let retry_after = rate_result.retry_after_seconds;
        let mut headers = HeaderMap::new();
        headers.insert(
            header::RETRY_AFTER,
            HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            headers,
            Json(json!({
                "error": "rate_limited",
                "retry_after_seconds": retry_after,
            })),
        )
            .into_response();
    }

    let project_id = match state.auth.validate(&write_key_str).await {
        Some(pid) => pid,
        None => {
            return json_response(
                StatusCode::UNAUTHORIZED,
                json!({"error": "invalid_write_key"}),
            );
        }
    };

    tracing::debug!(write_key = %write_key_str, project_id = %project_id, "write key validated");

    let payload: BatchPayload = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "validation_failed",
                    "detail": "request body is empty or malformed",
                }),
            );
        }
    };

    if let Err(e) = validate_batch(&payload) {
        let (status, body) = validation_to_status(&e);
        return json_response(status, body);
    }

    let ip = extract_ip(&headers);

    let raw_events = stamp_events(payload.batch, &project_id, &payload.sent_at, &ip);

    let ingested = raw_events.len();

    if let Err(e) = state.kafka.send_raw(&project_id, &raw_events).await {
        tracing::error!(error = %e, project_id = %project_id, "kafka send failed");
        return json_response(
            StatusCode::BAD_GATEWAY,
            json!({
                "error": "upstream_unavailable",
                "detail": "kafka_down",
            }),
        );
    }

    json_response(
        StatusCode::OK,
        json!({"status": "ok", "ingested": ingested}),
    )
}

fn validation_to_status(err: &ValidationError) -> (StatusCode, serde_json::Value) {
    match err {
        ValidationError::BatchTooLarge { got: _, max_events } => (
            StatusCode::PAYLOAD_TOO_LARGE,
            json!({
                "error": "batch_too_large",
                "max_events": max_events,
                "max_bytes": 1_048_576,
            }),
        ),
        ValidationError::InvalidEvent { index, message } => (
            StatusCode::BAD_REQUEST,
            json!({
                "error": "validation_failed",
                "detail": format!("event {}: {}", index, message),
            }),
        ),
    }
}
