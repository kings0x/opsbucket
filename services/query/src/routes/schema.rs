use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::auth::secret_key::check_auth;
use crate::ch::client;
use crate::{AppError, AppState, SchemaEventRow, SchemaPropertyRow, SchemaResponse};

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaParams {
    pub project_id: String,
}

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(params): Query<SchemaParams>,
) -> Result<Json<SchemaResponse>, AppError> {
    check_auth(&headers, &state.secret_keys).await.map_err(AppError::unauthorized)?;

    let events_sql = "
        SELECT
            event_name,
            count() AS volume,
            formatDateTime(toDate(min(timestamp)), '%Y-%m-%d') AS first_seen
        FROM events
        WHERE project_id = ?
          AND timestamp >= now() - INTERVAL 30 DAY
        GROUP BY event_name
        ORDER BY volume DESC
    ";
    let events_plan = client::QueryPlan::new(
        events_sql.to_string(),
        vec![client::QueryParam::String(params.project_id.clone())],
    );
    let event_rows: Vec<SchemaEventRow> =
        client::query_plan(&state.ch_client, &events_plan, state.config.query_timeout_seconds)
            .await
            .map_err(AppError::from)?;

    let props_sql = "
        SELECT
            event_name,
            arrayJoin(mapKeys(properties)) AS prop_key
        FROM events
        WHERE project_id = ?
        GROUP BY event_name, prop_key
        ORDER BY event_name, prop_key
    ";
    let props_plan = client::QueryPlan::new(
        props_sql.to_string(),
        vec![client::QueryParam::String(params.project_id)],
    );
    let prop_rows: Vec<SchemaPropertyRow> =
        client::query_plan(&state.ch_client, &props_plan, state.config.query_timeout_seconds)
            .await
            .map_err(AppError::from)?;

    let events: Vec<_> = event_rows
        .into_iter()
        .map(|r| crate::SchemaEvent {
            name: r.event_name,
            volume: r.volume,
            first_seen: r.first_seen,
        })
        .collect();

    let mut properties: HashMap<String, Vec<String>> = HashMap::new();
    for r in prop_rows {
        properties
            .entry(r.event_name)
            .or_default()
            .push(r.prop_key);
    }

    Ok(Json(SchemaResponse { events, properties }))
}
