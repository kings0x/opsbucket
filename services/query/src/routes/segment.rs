use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::auth::secret_key::check_auth;
use crate::cache;
use crate::ch::client;
use crate::queries::{builder, segment};
use crate::{AppError, AppState, SegmentResponse, SegmentRow, SegmentSpec};

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(spec): Json<SegmentSpec>,
) -> Result<Json<SegmentResponse>, AppError> {
    check_auth(&headers, &state.secret_key).map_err(AppError::unauthorized)?;
    builder::validate_segment(&spec).map_err(AppError::invalid_request)?;

    let cache_key = cache::cache_key(&spec);
    let mut redis = state.redis.clone();
    if let Some(cached) = cache::get::<SegmentResponse>(&mut redis, &cache_key)
        .await
        .map_err(AppError::from_anyhow)?
    {
        return Ok(Json(cached));
    }

    let (subqueries, limit) = segment::build(&spec).map_err(AppError::invalid_request)?;

    let mut result_sets = Vec::new();
    for sql in &subqueries {
        let rows: Vec<SegmentRow> =
            client::query(&state.ch_client, sql, state.config.query_timeout_seconds)
                .await
                .map_err(AppError::from)?;
        let set: HashSet<String> = rows.into_iter().map(|r| r.user_key).collect();
        result_sets.push(set);
    }

    let intersected = segment::intersect(result_sets);
    let total = intersected.len() as u64;
    let truncated = total > limit;

    let users: Vec<String> = intersected.into_iter().take(limit as usize).collect();

    let response = SegmentResponse {
        users,
        total,
        truncated,
    };

    let cloned = response.clone();
    if let Err(e) = cache::set(
        &mut redis,
        &cache_key,
        &cloned,
        state.config.query_cache_ttl_seconds,
    )
    .await
    {
        tracing::warn!(error = %e, "failed to cache segment result");
    }

    Ok(Json(response))
}
