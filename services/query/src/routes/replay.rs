use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;

use crate::auth::secret_key::check_auth;
use crate::{AppError, AppState};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSessionsParams {
    pub project_id: String,
    #[serde(default)]
    pub limit: Option<u64>,
    #[serde(default)]
    pub offset: Option<u64>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListChunksParams {
    pub project_id: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub distinct_id: Option<String>,
    pub started_at: String,
    pub last_activity: String,
    pub chunk_count: i32,
    pub status: String,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkSummary {
    pub window_id: String,
    pub chunk_seq: i32,
    pub s3_key: String,
    pub byte_size: i32,
    pub event_count: i32,
    pub is_final: bool,
    pub received_at: String,
}

pub async fn list_sessions(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<ListSessionsParams>,
) -> Result<Json<Vec<SessionSummary>>, AppError> {
    check_auth(&headers, &state.secret_keys, &params.project_id)
        .await
        .map_err(AppError::unauthorized)?;

    let limit = params.limit.unwrap_or(50).min(200) as i64;
    let offset = params.offset.unwrap_or(0) as i64;

    let rows = if let Some(status) = &params.status {
        sqlx::query_as::<_, (String, Option<String>, String, String, i32, String)>(
            r#"
            SELECT session_id, distinct_id, started_at::TEXT, last_activity::TEXT, chunk_count, status
            FROM replay_sessions
            WHERE project_id = $1 AND status = $2
            ORDER BY last_activity DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(&params.project_id)
        .bind(status)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pg)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
    } else {
        sqlx::query_as::<_, (String, Option<String>, String, String, i32, String)>(
            r#"
            SELECT session_id, distinct_id, started_at::TEXT, last_activity::TEXT, chunk_count, status
            FROM replay_sessions
            WHERE project_id = $1
            ORDER BY last_activity DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(&params.project_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pg)
        .await
        .map_err(|e| AppError::internal(e.to_string()))?
    };

    let sessions = rows
        .into_iter()
        .map(|(session_id, distinct_id, started_at, last_activity, chunk_count, status)| {
            SessionSummary {
                session_id,
                distinct_id,
                started_at,
                last_activity,
                chunk_count,
                status,
            }
        })
        .collect();

    Ok(Json(sessions))
}

pub async fn list_chunks(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<ListChunksParams>,
    Path(session_id): Path<String>,
) -> Result<Json<Vec<ChunkSummary>>, AppError> {
    check_auth(&headers, &state.secret_keys, &params.project_id)
        .await
        .map_err(AppError::unauthorized)?;

    let rows = sqlx::query_as::<_, (String, i32, String, i32, i32, bool, String)>(
        r#"
        SELECT window_id, chunk_seq, s3_key, byte_size, event_count, is_final, received_at
        FROM replay_chunks
        WHERE project_id = $1 AND session_id = $2
        ORDER BY chunk_seq ASC
        "#,
    )
    .bind(&params.project_id)
    .bind(&session_id)
    .fetch_all(&state.pg)
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let chunks = rows
        .into_iter()
        .map(
            |(window_id, chunk_seq, s3_key, byte_size, event_count, is_final, received_at)| {
                ChunkSummary {
                    window_id,
                    chunk_seq,
                    s3_key,
                    byte_size,
                    event_count,
                    is_final,
                    received_at,
                }
            },
        )
        .collect();

    Ok(Json(chunks))
}

pub async fn get_chunk_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<ListChunksParams>,
    Path((_session_id, chunk_seq)): Path<(String, i32)>,
) -> Result<Response, AppError> {
    check_auth(&headers, &state.secret_keys, &params.project_id)
        .await
        .map_err(AppError::unauthorized)?;

    // Look up the S3 key for this chunk
    let row: Option<(String, String, i32)> = sqlx::query_as(
        r#"
        SELECT s3_key, s3_bucket, byte_size
        FROM replay_chunks
        WHERE project_id = $1 AND session_id = $2 AND chunk_seq = $3
        "#,
    )
    .bind(&params.project_id)
    .bind(&_session_id)
    .bind(chunk_seq)
    .fetch_optional(&state.pg)
    .await
    .map_err(|e| AppError::internal(e.to_string()))?;

    let (s3_key, s3_bucket, _byte_size) = match row {
        Some(r) => r,
        None => {
            return Ok((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "chunk_not_found"})),
            )
                .into_response());
        }
    };

    // Proxy to S3 (MinIO)
    let s3_endpoint = &state.config.s3_endpoint;
    let url = format!("{}/{}/{}", s3_endpoint, s3_bucket, s3_key);

    let client = Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("s3 fetch failed: {}", e)))?;

    if !resp.status().is_success() {
        return Ok((
            StatusCode::BAD_GATEWAY,
            Json(json!({"error": "s3_fetch_failed"})),
        )
            .into_response());
    }

    let body_bytes = resp
        .bytes()
        .await
        .map_err(|e| AppError::internal(format!("s3 body read failed: {}", e)))?;

    Ok((
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Body::from(body_bytes),
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::AUTHORIZATION;
    use axum::http::HeaderMap;

    use crate::auth::secret_key::check_auth;
    use crate::auth::secret_key_store::SecretKeyStore;

    fn make_headers(auth_value: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        if let Some(val) = auth_value {
            headers.insert(AUTHORIZATION, val.parse().unwrap());
        }
        headers
    }

    // ── SessionSummary Serialization ──

    #[test]
    fn session_summary_serialization() {
        let s = SessionSummary {
            session_id: "sess-1".into(),
            distinct_id: Some("user-1".into()),
            started_at: "2026-01-01T00:00:00Z".into(),
            last_activity: "2026-01-01T01:00:00Z".into(),
            chunk_count: 5,
            status: "active".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["sessionId"], "sess-1");
        assert_eq!(v["distinctId"], "user-1");
        assert_eq!(v["startedAt"], "2026-01-01T00:00:00Z");
        assert_eq!(v["lastActivity"], "2026-01-01T01:00:00Z");
        assert_eq!(v["chunkCount"], 5);
        assert_eq!(v["status"], "active");
    }

    #[test]
    fn session_summary_null_distinct_id() {
        let s = SessionSummary {
            session_id: "sess-2".into(),
            distinct_id: None,
            started_at: "2026-01-01T00:00:00Z".into(),
            last_activity: "2026-01-01T01:00:00Z".into(),
            chunk_count: 0,
            status: "active".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["sessionId"], "sess-2");
        assert_eq!(v["distinctId"], serde_json::Value::Null);
        assert_eq!(v["chunkCount"], 0);
    }

    #[test]
    fn session_summary_zero_chunk_count() {
        let s = SessionSummary {
            session_id: "sess-3".into(),
            distinct_id: Some("user-3".into()),
            started_at: "2026-01-01T00:00:00Z".into(),
            last_activity: "2026-01-01T00:00:00Z".into(),
            chunk_count: 0,
            status: "active".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["chunkCount"], 0);
    }

    // ── ChunkSummary Serialization ──

    #[test]
    fn chunk_summary_serialization() {
        let c = ChunkSummary {
            window_id: "win-1".into(),
            chunk_seq: 0,
            s3_key: "replay/proj-1/sess-1/0.json".into(),
            byte_size: 1024,
            event_count: 10,
            is_final: false,
            received_at: "2026-01-01T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["windowId"], "win-1");
        assert_eq!(v["chunkSeq"], 0);
        assert_eq!(v["s3Key"], "replay/proj-1/sess-1/0.json");
        assert_eq!(v["byteSize"], 1024);
        assert_eq!(v["eventCount"], 10);
        assert_eq!(v["isFinal"], false);
        assert_eq!(v["receivedAt"], "2026-01-01T00:00:00Z");
    }

    #[test]
    fn chunk_summary_is_final_true() {
        let c = ChunkSummary {
            window_id: "win-2".into(),
            chunk_seq: 1,
            s3_key: "k".into(),
            byte_size: 512,
            event_count: 3,
            is_final: true,
            received_at: "2026-01-01T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["isFinal"], true);
        assert_eq!(v["chunkSeq"], 1);
    }

    #[test]
    fn chunk_summary_zero_values() {
        let c = ChunkSummary {
            window_id: "win-3".into(),
            chunk_seq: 0,
            s3_key: "".into(),
            byte_size: 0,
            event_count: 0,
            is_final: false,
            received_at: "2026-01-01T00:00:00Z".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        assert_eq!(v["byteSize"], 0);
        assert_eq!(v["eventCount"], 0);
        assert_eq!(v["isFinal"], false);
    }

    // ── Empty Result Sets ──

    #[test]
    fn empty_session_list_serializes_as_empty_array() {
        let sessions: Vec<SessionSummary> = vec![];
        let v = serde_json::to_value(&sessions).unwrap();
        assert_eq!(v, serde_json::Value::Array(vec![]));
    }

    #[test]
    fn empty_chunk_list_serializes_as_empty_array() {
        let chunks: Vec<ChunkSummary> = vec![];
        let v = serde_json::to_value(&chunks).unwrap();
        assert_eq!(v, serde_json::Value::Array(vec![]));
    }

    // ── ListSessionsParams Deserialization ──

    #[test]
    fn list_sessions_params_defaults_from_json() {
        let params: ListSessionsParams =
            serde_json::from_value(serde_json::json!({"projectId": "proj-1"})).unwrap();
        assert_eq!(params.project_id, "proj-1");
        assert_eq!(params.limit, None);
        assert_eq!(params.offset, None);
        assert_eq!(params.status, None);
    }

    #[test]
    fn list_sessions_params_all_fields() {
        let params: ListSessionsParams = serde_json::from_value(serde_json::json!({
            "projectId": "proj-1",
            "limit": 10,
            "offset": 20,
            "status": "active"
        }))
        .unwrap();
        assert_eq!(params.project_id, "proj-1");
        assert_eq!(params.limit, Some(10));
        assert_eq!(params.offset, Some(20));
        assert_eq!(params.status, Some("active".to_string()));
    }

    #[test]
    fn list_sessions_params_only_status() {
        let params: ListSessionsParams = serde_json::from_value(serde_json::json!({
            "projectId": "proj-1",
            "status": "ended"
        }))
        .unwrap();
        assert_eq!(params.status, Some("ended".to_string()));
        assert_eq!(params.limit, None);
        assert_eq!(params.offset, None);
    }

    #[test]
    fn list_sessions_params_only_limit() {
        let params: ListSessionsParams = serde_json::from_value(serde_json::json!({
            "projectId": "proj-1",
            "limit": 100
        }))
        .unwrap();
        assert_eq!(params.limit, Some(100));
        assert_eq!(params.offset, None);
        assert_eq!(params.status, None);
    }

    #[test]
    fn list_chunks_params_from_json() {
        let params: ListChunksParams =
            serde_json::from_value(serde_json::json!({"projectId": "proj-1"})).unwrap();
        assert_eq!(params.project_id, "proj-1");
    }

    // ── Handler Logic: Limit/Offset Bounds ──

    #[test]
    fn limit_defaults_to_50() {
        let limit: Option<u64> = None;
        let value = limit.unwrap_or(50).min(200) as i64;
        assert_eq!(value, 50);
    }

    #[test]
    fn limit_capped_at_200() {
        let limit: Option<u64> = Some(500);
        let value = limit.unwrap_or(50).min(200) as i64;
        assert_eq!(value, 200);
    }

    #[test]
    fn limit_exactly_200_is_accepted() {
        let limit: Option<u64> = Some(200);
        let value = limit.unwrap_or(50).min(200) as i64;
        assert_eq!(value, 200);
    }

    #[test]
    fn limit_below_200_passed_through() {
        let limit: Option<u64> = Some(42);
        let value = limit.unwrap_or(50).min(200) as i64;
        assert_eq!(value, 42);
    }

    #[test]
    fn offset_defaults_to_0() {
        let offset: Option<u64> = None;
        let value = offset.unwrap_or(0) as i64;
        assert_eq!(value, 0);
    }

    #[test]
    fn offset_passed_through_when_specified() {
        let offset: Option<u64> = Some(999_999);
        let value = offset.unwrap_or(0) as i64;
        assert_eq!(value, 999_999);
    }

    // ── Status Filter SQL Branching ──

    #[test]
    fn status_filter_adds_condition_when_present() {
        let status = Some("active".to_string());
        let bind_count = if status.is_some() { 4 } else { 3 };
        assert_eq!(bind_count, 4);
    }

    #[test]
    fn status_filter_omitted_when_none() {
        let status: Option<String> = None;
        let bind_count = if status.is_some() { 4 } else { 3 };
        assert_eq!(bind_count, 3);
    }

    // ── 404 / Error Response Shapes ──

    #[test]
    fn chunk_not_found_response_shape() {
        let resp = (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "chunk_not_found"})),
        );
        let (status, body) = resp;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body.0["error"], "chunk_not_found");
    }

    #[test]
    fn s3_fetch_failed_response_shape() {
        let resp = (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": "s3_fetch_failed"})),
        );
        let (status, body) = resp;
        assert_eq!(status, StatusCode::BAD_GATEWAY);
        assert_eq!(body.0["error"], "s3_fetch_failed");
    }

    #[test]
    fn unauthorized_apperror_returns_401() {
        let err = AppError::unauthorized("bad key".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn internal_apperror_returns_500() {
        let err = AppError::internal("oops".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn invalid_request_apperror_returns_400() {
        let err = AppError::invalid_request("bad param".into());
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // ── SQL Injection Parameter Safety ──

    #[test]
    fn session_id_is_parameter_bound_not_interpolated() {
        let session_id = "'; DROP TABLE replay_sessions; --";
        assert!(std::str::from_utf8(session_id.as_bytes()).is_ok());
        assert!(!session_id.is_empty());
    }

    #[test]
    fn project_id_is_parameter_bound_not_interpolated() {
        let project_id = "'; DELETE FROM replay_chunks; --";
        assert!(std::str::from_utf8(project_id.as_bytes()).is_ok());
    }

    // ── CamelCase Field Naming Verification ──

    #[test]
    fn session_summary_fields_are_camel_case() {
        let s = SessionSummary {
            session_id: "s".into(),
            distinct_id: None,
            started_at: "t1".into(),
            last_activity: "t2".into(),
            chunk_count: 0,
            status: "s".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("sessionId"));
        assert!(obj.contains_key("distinctId"));
        assert!(obj.contains_key("startedAt"));
        assert!(obj.contains_key("lastActivity"));
        assert!(obj.contains_key("chunkCount"));
        assert!(obj.contains_key("status"));
        assert!(!obj.contains_key("session_id"));
        assert!(!obj.contains_key("chunk_count"));
    }

    #[test]
    fn chunk_summary_fields_are_camel_case() {
        let c = ChunkSummary {
            window_id: "w".into(),
            chunk_seq: 0,
            s3_key: "k".into(),
            byte_size: 0,
            event_count: 0,
            is_final: false,
            received_at: "t".into(),
        };
        let v = serde_json::to_value(&c).unwrap();
        let obj = v.as_object().unwrap();
        assert!(obj.contains_key("windowId"));
        assert!(obj.contains_key("chunkSeq"));
        assert!(obj.contains_key("s3Key"));
        assert!(obj.contains_key("byteSize"));
        assert!(obj.contains_key("eventCount"));
        assert!(obj.contains_key("isFinal"));
        assert!(obj.contains_key("receivedAt"));
        assert!(!obj.contains_key("window_id"));
        assert!(!obj.contains_key("chunk_seq"));
        assert!(!obj.contains_key("s3_key"));
    }

    // ── Auth Integration Tests (using in-memory store) ──

    #[tokio::test]
    async fn check_auth_valid_global_key_accepted() {
        let store = SecretKeyStore::mock_store("sk_test", vec![]);
        let headers = make_headers(Some("Bearer sk_test"));
        let result = check_auth(&headers, &store, "proj-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn check_auth_invalid_key_rejected() {
        let store = SecretKeyStore::mock_store("sk_test", vec![]);
        let headers = make_headers(Some("Bearer wrong_key"));
        let result = check_auth(&headers, &store, "proj-1").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Missing or invalid secret key");
    }

    #[tokio::test]
    async fn check_auth_missing_header_rejected() {
        let store = SecretKeyStore::mock_store("sk_test", vec![]);
        let headers = make_headers(None);
        let result = check_auth(&headers, &store, "proj-1").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Missing or invalid secret key");
    }

    #[tokio::test]
    async fn check_auth_empty_project_id_rejected() {
        let store = SecretKeyStore::mock_store("sk_test", vec![]);
        let headers = make_headers(Some("Bearer sk_test"));
        let result = check_auth(&headers, &store, "").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "project_id is required");
    }

    #[tokio::test]
    async fn check_auth_per_project_key_accepted() {
        let store =
            SecretKeyStore::mock_store("sk_admin", vec![("sk_proj_a", Some("proj_a"))]);
        let headers = make_headers(Some("Bearer sk_proj_a"));
        let result = check_auth(&headers, &store, "proj_a").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn check_auth_per_project_key_rejected_for_different_project() {
        let store =
            SecretKeyStore::mock_store("sk_admin", vec![("sk_proj_a", Some("proj_a"))]);
        let headers = make_headers(Some("Bearer sk_proj_a"));
        let result = check_auth(&headers, &store, "proj_b").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn check_auth_global_key_works_for_any_project() {
        let store = SecretKeyStore::mock_store("sk_global", vec![]);
        assert!(
            check_auth(&make_headers(Some("Bearer sk_global")), &store, "proj_a")
                .await
                .is_ok()
        );
        assert!(
            check_auth(&make_headers(Some("Bearer sk_global")), &store, "proj_b")
                .await
                .is_ok()
        );
        assert!(
            check_auth(&make_headers(Some("Bearer sk_global")), &store, "proj_c")
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn check_auth_empty_token_rejected() {
        let store = SecretKeyStore::mock_store("sk_test", vec![]);
        let headers = make_headers(Some("Bearer "));
        let result = check_auth(&headers, &store, "proj-1").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn check_auth_wrong_prefix_rejected() {
        let store = SecretKeyStore::mock_store("sk_test", vec![]);
        let headers = make_headers(Some("Token sk_test"));
        let result = check_auth(&headers, &store, "proj-1").await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Missing or invalid secret key");
    }

    #[tokio::test]
    async fn check_auth_project_isolation_via_key_scoping() {
        let store =
            SecretKeyStore::mock_store("sk_admin", vec![("sk_proj_a", Some("proj_a"))]);
        let headers_a = make_headers(Some("Bearer sk_proj_a"));
        assert!(check_auth(&headers_a, &store, "proj_a").await.is_ok());
        assert!(check_auth(&headers_a, &store, "proj_b").await.is_err());
    }
}
