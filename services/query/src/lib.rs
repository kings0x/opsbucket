pub mod auth;
pub mod cache;
pub mod ch;
pub mod config;
pub mod identity;
pub mod queries;
pub mod routes;

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use clickhouse::Row;

pub struct AppState {
    pub secret_key: String,
    pub pg: sqlx::PgPool,
    pub redis: redis::aio::ConnectionManager,
    pub ch_client: ::clickhouse::Client,
    pub config: config::Config,
}

pub type SharedState = Arc<AppState>;

// ── AppError ───────────────────────────────────────────────────────────────────────

pub enum AppError {
    Unauthorized(String),
    InvalidRequest(String),
    QueryTimeout,
    Internal(String),
}

impl AppError {
    pub fn unauthorized(detail: String) -> Self {
        AppError::Unauthorized(detail)
    }

    pub fn invalid_request(detail: String) -> Self {
        AppError::InvalidRequest(detail)
    }

    pub fn internal(detail: String) -> Self {
        AppError::Internal(detail)
    }

    pub fn from_anyhow(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, detail) = match self {
            AppError::Unauthorized(d) => (StatusCode::UNAUTHORIZED, "unauthorized", d),
            AppError::InvalidRequest(d) => (StatusCode::BAD_REQUEST, "invalid_request", d),
            AppError::QueryTimeout => (
                StatusCode::REQUEST_TIMEOUT,
                "query_timeout",
                "ClickHouse query exceeded timeout".into(),
            ),
            AppError::Internal(d) => {
                tracing::error!(detail = %d, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An unexpected error occurred".into(),
                )
            }
        };
        (
            status,
            Json(ErrorResponse {
                error: code.into(),
                detail,
            }),
        )
            .into_response()
    }
}

impl From<crate::ch::client::QueryError> for AppError {
    fn from(e: crate::ch::client::QueryError) -> Self {
        match e {
            crate::ch::client::QueryError::Timeout => AppError::QueryTimeout,
            _ => AppError::Internal(e.to_string()),
        }
    }
}

// ── Shared types ───────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FunnelSpec {
    pub project_id: String,
    pub steps: Vec<String>,
    pub window_seconds: u64,
    pub date_range: DateRange,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionSpec {
    pub project_id: String,
    pub event_name: String,
    pub interval: String,
    pub periods: u64,
    pub date_range: DateRange,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentSpec {
    pub project_id: String,
    pub conditions: Vec<SegmentCondition>,
    pub limit: u64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    #[serde(default)]
    pub event_name: Option<String>,
    pub op: String,
    pub value: serde_json::Value,
    #[serde(default)]
    pub within_days: Option<u64>,
    #[serde(default)]
    pub key: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunnelResponse {
    pub steps: Vec<FunnelStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_at: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FunnelStep {
    pub name: String,
    pub users: u64,
    pub conversion_rate: f64,
    pub overall_rate: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RetentionResponse {
    pub cohorts: Vec<Cohort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_at: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Cohort {
    pub cohort_date: String,
    pub initial_users: u64,
    pub periods: Vec<CohortPeriod>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CohortPeriod {
    pub period: u64,
    pub users: u64,
    pub rate: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SegmentResponse {
    pub users: Vec<String>,
    pub total: u64,
    pub truncated: bool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventsResponse {
    pub events: Vec<EventRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub event_id: String,
    pub event_name: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub timestamp: String,
    pub properties: HashMap<String, String>,
    pub page_url: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub error: String,
    pub detail: String,
}

// ClickHouse row types

#[derive(Debug, Row, serde::Deserialize)]
pub struct FunnelRow {
    pub level: u8,
    pub users_reached: u64,
}

#[derive(Debug, Row, serde::Deserialize)]
pub struct RetentionRow {
    pub cohort_date: i32,
    pub period: i32,
    pub users: u64,
}

#[derive(Debug, Row, serde::Deserialize)]
pub struct SegmentRow {
    pub user_key: String,
}

#[derive(Debug, Row, serde::Deserialize)]
pub struct ClickHouseEventRow {
    pub event_id: String,
    pub event_name: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub timestamp: u32,
    pub page_url: String,
    pub page_referrer: String,
    pub user_agent: String,
    pub properties: Vec<(String, String)>,
}

#[derive(Debug, sqlx::FromRow)]
pub struct IdentityAlias {
    pub project_id: String,
    pub anonymous_id: String,
    pub user_id: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Cursor {
    pub ts: chrono::DateTime<chrono::Utc>,
    pub id: String,
}
