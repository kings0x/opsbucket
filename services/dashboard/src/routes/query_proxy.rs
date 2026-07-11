use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::sync::Arc;

use crate::AppState;

const PROXY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(25);

async fn proxy_get(state: &Arc<AppState>, path: &str, query_string: &str) -> Response {
    let url = format!("{}/v1/query/{}{}", state.query_url, path, query_string);

    match state
        .http_client
        .get(&url)
        .header(
            "Authorization",
            format!("Bearer {}", state.query_secret_key),
        )
        .timeout(PROXY_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.bytes().await.unwrap_or_default();
            (status, body).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": "proxy_error",
                "detail": e.to_string()
            })),
        )
            .into_response(),
    }
}

async fn proxy_post(state: &Arc<AppState>, path: &str, body: Bytes) -> Response {
    let url = format!("{}/v1/query/{}", state.query_url, path);

    match state
        .http_client
        .post(&url)
        .header(
            "Authorization",
            format!("Bearer {}", state.query_secret_key),
        )
        .header("Content-Type", "application/json")
        .body(body)
        .timeout(PROXY_TIMEOUT)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body = resp.bytes().await.unwrap_or_default();
            (status, body).into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": "proxy_error",
                "detail": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn funnel_handler(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    proxy_post(&state, "funnel", body).await
}

pub async fn retention_handler(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    proxy_post(&state, "retention", body).await
}

pub async fn segment_handler(State(state): State<Arc<AppState>>, body: Bytes) -> Response {
    proxy_post(&state, "segment", body).await
}

pub async fn schema_handler(
    State(state): State<Arc<AppState>>,
    query_string: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let qs = query_string
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding(v)))
        .collect::<Vec<_>>()
        .join("&");
    let qs = if qs.is_empty() {
        String::new()
    } else {
        format!("?{}", qs)
    };
    proxy_get(&state, "schema", &qs).await
}

pub async fn stats_handler(
    State(state): State<Arc<AppState>>,
    query_string: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let qs = query_string
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding(v)))
        .collect::<Vec<_>>()
        .join("&");
    let qs = if qs.is_empty() {
        String::new()
    } else {
        format!("?{}", qs)
    };
    proxy_get(&state, "stats", &qs).await
}

pub async fn events_handler(
    State(state): State<Arc<AppState>>,
    query_string: axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Response {
    let qs = query_string
        .iter()
        .map(|(k, v)| format!("{}={}", k, urlencoding(v)))
        .collect::<Vec<_>>()
        .join("&");
    let qs = if qs.is_empty() {
        String::new()
    } else {
        format!("?{}", qs)
    };
    proxy_get(&state, "events", &qs).await
}

fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{:02X}", b));
            }
        }
    }
    out
}
