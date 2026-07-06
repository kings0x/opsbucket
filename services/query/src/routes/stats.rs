use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::auth::secret_key::check_auth;
use crate::ch::client;
use crate::{AppError, AppState, StatsResponse, StatsRow};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsParams {
    pub project_id: String,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<StatsParams>,
) -> Result<Json<StatsResponse>, AppError> {
    check_auth(&headers, &state.secret_keys).await.map_err(AppError::unauthorized)?;

    let sql = "
        SELECT
            countIf(timestamp >= now() - INTERVAL 30 DAY) AS events_last_30,
            countIf(timestamp >= now() - INTERVAL 60 DAY AND timestamp < now() - INTERVAL 30 DAY) AS events_prev_30
        FROM events
        WHERE project_id = ?
    ";
    let plan = client::QueryPlan::new(
        sql.to_string(),
        vec![client::QueryParam::String(params.project_id)],
    );
    let mut rows: Vec<StatsRow> =
        client::query_plan(&state.ch_client, &plan, state.config.query_timeout_seconds)
            .await
            .map_err(AppError::from)?;

    let row = rows.pop().unwrap_or(StatsRow {
        events_last_30: 0,
        events_prev_30: 0,
    });

    let trend_percent = if row.events_prev_30 > 0 {
        ((row.events_last_30 as f64 - row.events_prev_30 as f64) / row.events_prev_30 as f64) * 100.0
    } else if row.events_last_30 > 0 {
        100.0
    } else {
        0.0
    };

    Ok(Json(StatsResponse {
        events_last_30_days: row.events_last_30,
        events_previous_30_days: row.events_prev_30,
        trend_percent: (trend_percent * 10.0).round() / 10.0,
    }))
}
