use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::header;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use opsbucket_shared::events::{ReplayBatch, ReplayBatchEnvelope};
use serde::Deserialize;
use serde_json::json;

use crate::AppState;

#[derive(Deserialize)]
pub struct ReplayQuery {
    write_key: Option<String>,
}

fn extract_write_key(headers: &HeaderMap, query: &ReplayQuery) -> Option<String> {
    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(key) = auth.strip_prefix("Bearer ") {
            return Some(key.to_string());
        }
    }
    query.write_key.clone()
}

fn json_response(status: StatusCode, value: serde_json::Value) -> Response {
    (status, Json(value)).into_response()
}

pub async fn post_replay(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ReplayQuery>,
    bytes: Bytes,
) -> Response {
    // 1. Extract write key
    let write_key_str = match extract_write_key(&headers, &query) {
        Some(k) => k,
        None => {
            return json_response(
                StatusCode::UNAUTHORIZED,
                json!({"error": "invalid_write_key"}),
            );
        }
    };

    // 2. Rate limit check (before auth)
    let rate_result = {
        let mut limiter = state.replay_rate_limiter.lock().unwrap();
        limiter.check(&write_key_str)
    };

    if !rate_result.allowed {
        let retry_after = rate_result.retry_after_seconds;
        let mut response_headers = HeaderMap::new();
        response_headers.insert(
            header::RETRY_AFTER,
            HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );
        return (
            StatusCode::TOO_MANY_REQUESTS,
            response_headers,
            Json(json!({
                "error": "rate_limited",
                "retry_after_seconds": retry_after,
            })),
        )
            .into_response();
    }

    // 3. Auth: validate write key -> project_id
    let project_id = match state.auth.validate(&write_key_str).await {
        Ok(Some(pid)) => pid,
        Ok(None) => {
            return json_response(
                StatusCode::UNAUTHORIZED,
                json!({"error": "invalid_write_key"}),
            );
        }
        Err(e) => {
            tracing::error!(error = %e, "write key validation failed for replay");
            return json_response(
                StatusCode::SERVICE_UNAVAILABLE,
                json!({"error": "auth_unavailable"}),
            );
        }
    };

    tracing::debug!(project_id = %project_id, "replay write key validated");

    // 4. Decompress if gzip
    let decompressed = if headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("gzip"))
        .unwrap_or(false)
    {
        // TODO: decompress gzip body in a follow-up if needed
        // For now, reject gzipped payloads with 415
        return json_response(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            json!({"error": "gzip_not_supported"}),
        );
    } else {
        bytes
    };

    // 5. Deserialize body as ReplayBatch
    let batch: ReplayBatch = match serde_json::from_slice(&decompressed) {
        Ok(b) => b,
        Err(e) => {
            return json_response(
                StatusCode::BAD_REQUEST,
                json!({
                    "error": "validation_failed",
                    "detail": format!("malformed replay payload: {}", e),
                }),
            );
        }
    };

    // 6. Validate required fields
    if batch.events.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "validation_failed",
                "detail": "events array must not be empty",
            }),
        );
    }

    if batch.session_id.is_empty() {
        return json_response(
            StatusCode::BAD_REQUEST,
            json!({
                "error": "validation_failed",
                "detail": "session_id is required",
            }),
        );
    }

    // 7. Build envelope with server-stamped fields
    let envelope = ReplayBatchEnvelope {
        project_id: project_id.clone(),
        received_at: Utc::now().to_rfc3339(),
        batch: ReplayBatch {
            project_id,
            ..batch
        },
    };

    let event_count = envelope.batch.events.len();

    // 8. Produce to Kafka
    if let Err(e) = state
        .kafka
        .send_replay(&envelope.project_id, &envelope)
        .await
    {
        tracing::error!(
            error = %e,
            project_id = %envelope.project_id,
            "replay kafka send failed"
        );
        return json_response(
            StatusCode::BAD_GATEWAY,
            json!({"error": "upstream_unavailable", "detail": "kafka_down"}),
        );
    }

    // 9. Return success
    json_response(
        StatusCode::OK,
        json!({"status": "ok", "ingested": event_count}),
    )
}
