use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
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
    if let Some(fwd) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
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

fn stamp_events(
    events: Vec<AnyEvent>,
    project_id: &str,
    ip: &str,
) -> Vec<RawEvent> {
    let received_at = Utc::now().to_rfc3339();
    events
        .into_iter()
        .map(|e| RawEvent::from_any(e, project_id.to_string(), received_at.clone(), ip.to_string()))
        .collect()
}

fn validation_to_status(err: &ValidationError) -> (StatusCode, serde_json::Value) {
    match err {
        ValidationError::BatchTooLarge { got: _, max_events } => {
            (StatusCode::PAYLOAD_TOO_LARGE, json!({
                "error": "batch_too_large",
                "max_events": max_events,
                "max_bytes": 1_048_576,
            }))
        }
        ValidationError::InvalidEvent { index, message } => {
            (StatusCode::BAD_REQUEST, json!({
                "error": "validation_failed",
                "detail": format!("event {}: {}", index, message),
            }))
        }
    }
}

pub async fn post_batch(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<BatchQuery>,
    bytes: Bytes,
) -> impl IntoResponse {
    let write_key_str = match extract_write_key(&headers, &query) {
        Some(k) => k,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid_write_key"})),
            );
        }
    };

    {
        let mut limiter = state.rate_limiter.lock().unwrap();
        if !limiter.check(&write_key_str) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({
                    "error": "rate_limited",
                    "retry_after_seconds": 60,
                })),
            );
        }
    }

    let project_id = match state.auth.validate(&write_key_str).await {
        Some(pid) => pid,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid_write_key"})),
            );
        }
    };

    let payload: BatchPayload = match serde_json::from_slice(&bytes) {
        Ok(p) => p,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "validation_failed",
                    "detail": "request body is empty or malformed",
                })),
            );
        }
    };

    if let Err(e) = validate_batch(&payload) {
        let (status, body) = validation_to_status(&e);
        return (status, Json(body));
    }

    let ip = extract_ip(&headers);

    let raw_events = stamp_events(payload.batch, &project_id, &ip);

    let ingested = raw_events.len();

    if let Err(e) = state.kafka.send_raw(&project_id, &raw_events).await {
        tracing::error!(error = %e, project_id = %project_id, "kafka send failed");
        return (
            StatusCode::BAD_GATEWAY,
            Json(json!({
                "error": "upstream_unavailable",
                "detail": "kafka_down",
            })),
        );
    }

    (
        StatusCode::OK,
        Json(json!({"status": "ok", "ingested": ingested})),
    )
}
