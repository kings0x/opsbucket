use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

pub async fn post_batch(State(_state): State<Arc<crate::AppState>>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "ok", "ingested": 0})))
}
