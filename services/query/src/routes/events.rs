use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;

use std::collections::HashMap;

use crate::auth::secret_key::check_auth;
use crate::ch::client;
use crate::identity;
use crate::queries::{builder, raw_events};
use crate::{AppError, AppState, ClickHouseEventRow, EventRow, EventsResponse};

#[derive(Deserialize)]
pub struct EventsParams {
    #[serde(rename = "projectId")]
    pub project_id: String,
    #[serde(rename = "eventName")]
    pub event_name: Option<String>,
    #[serde(rename = "userId")]
    pub user_id: Option<String>,
    pub limit: Option<u64>,
    pub cursor: Option<String>,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<EventsParams>,
) -> Result<Json<EventsResponse>, AppError> {
    check_auth(&headers, &state.secret_key).map_err(AppError::unauthorized)?;

    builder::validate_string(&params.project_id, "projectId").map_err(AppError::invalid_request)?;
    if let Some(event_name) = params.event_name.as_deref() {
        builder::validate_string(event_name, "eventName").map_err(AppError::invalid_request)?;
    }
    if let Some(user_id) = params.user_id.as_deref() {
        builder::validate_string(user_id, "userId").map_err(AppError::invalid_request)?;
    }
    let limit = match params.limit {
        Some(limit @ 1..=200) => limit,
        Some(_) => {
            return Err(AppError::invalid_request(
                "limit must be between 1 and 200".into(),
            ));
        }
        None => 50,
    };
    let cursor = match &params.cursor {
        Some(c) => Some(raw_events::decode_cursor(c).map_err(AppError::invalid_request)?),
        None => None,
    };

    let alias_anonymous_ids = match params.user_id.as_deref() {
        Some(user_id) => sqlx::query_scalar::<_, String>(
            "SELECT anonymous_id
             FROM identity_aliases
             WHERE project_id = $1 AND user_id = $2",
        )
        .bind(&params.project_id)
        .bind(user_id)
        .fetch_all(&state.pg)
        .await
        .map_err(|e| AppError::from_anyhow(e.into()))?,
        None => Vec::new(),
    };

    let plan = raw_events::build_plan(
        &params.project_id,
        params.event_name.as_deref(),
        params.user_id.as_deref(),
        &alias_anonymous_ids,
        cursor.as_ref(),
        limit,
    );

    let ch_rows: Vec<ClickHouseEventRow> =
        client::query_plan(&state.ch_client, &plan, state.config.query_timeout_seconds)
            .await
            .map_err(AppError::from)?;

    let mut events: Vec<EventRow> = ch_rows
        .into_iter()
        .map(|r| {
            let ts = chrono::DateTime::from_timestamp(r.timestamp as i64, 0).unwrap_or_default();
            EventRow {
                project_id: r.project_id,
                event_id: r.event_id,
                event_name: r.event_name,
                anonymous_id: r.anonymous_id,
                user_id: r.user_id,
                timestamp: ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                properties: r.properties.into_iter().collect::<HashMap<_, _>>(),
                page_url: if r.page_url.is_empty() {
                    "".into()
                } else {
                    r.page_url
                },
            }
        })
        .collect();

    events = identity::patch_identity(events, &state.pg)
        .await
        .map_err(AppError::from_anyhow)?;

    let next_cursor = if events.len() as u64 == limit {
        let last = events.last().unwrap();
        let ts = chrono::DateTime::parse_from_rfc3339(&last.timestamp)
            .map_err(|e| AppError::internal(e.to_string()))?
            .with_timezone(&chrono::Utc);
        Some(raw_events::encode_cursor(&ts, &last.event_id))
    } else {
        None
    };

    Ok(Json(EventsResponse {
        events,
        next_cursor,
    }))
}
