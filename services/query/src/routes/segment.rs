use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::auth::secret_key::check_auth;
use crate::cache;
use crate::ch::client;
use crate::queries::{builder, segment};
use crate::{
    AppError, AppState, SegmentResponse, SegmentRow, SegmentSpec,
};

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<SegmentResponse>, AppError> {
    check_auth(&headers, &state.secret_keys).await.map_err(AppError::unauthorized)?;
    let spec: SegmentSpec = serde_json::from_slice(&body)
        .map_err(|_| AppError::invalid_request("request body is empty or malformed".into()))?;
    builder::validate_segment(&spec).map_err(AppError::invalid_request)?;

    let cache_key = cache::cache_key(&spec);
    let mut redis = state.redis.clone();
    if let Some(mut cached) = cache::get::<SegmentResponse>(&mut redis, &cache_key)
        .await
        .map_err(AppError::from_anyhow)?
    {
        cached.cached_at = Some(chrono::Utc::now().to_rfc3339());
        return Ok(Json(cached));
    }

    let (plans, limit) = segment::build_plan(&spec).map_err(AppError::invalid_request)?;

    let mut result_sets = Vec::new();
    for plan in &plans {
        let rows: Vec<SegmentRow> =
            client::query_plan(&state.ch_client, plan, state.config.query_timeout_seconds)
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
        cached_at: None,
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
