use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::auth::secret_key::check_auth;
use crate::cache;
use crate::ch::client;
use crate::queries::{builder, funnel};
use crate::{AppError, AppState, FunnelResponse, FunnelRow, FunnelSpec, FunnelStep};

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(spec): Json<FunnelSpec>,
) -> Result<Json<FunnelResponse>, AppError> {
    check_auth(&headers, &state.secret_key).map_err(AppError::unauthorized)?;
    builder::validate_funnel(&spec).map_err(AppError::invalid_request)?;

    let cache_key = cache::cache_key(&spec);
    let mut redis = state.redis.clone();
    if let Some(cached) = cache::get::<FunnelResponse>(&mut redis, &cache_key)
        .await
        .map_err(AppError::from_anyhow)?
    {
        return Ok(Json(cached));
    }

    let sql = funnel::build(&spec);
    let rows: Vec<FunnelRow> =
        client::query(&state.ch_client, &sql, state.config.query_timeout_seconds)
            .await
            .map_err(AppError::from)?;

    let steps = build_steps(&spec.steps, &rows);
    let response = FunnelResponse {
        steps,
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
        tracing::warn!(error = %e, "failed to cache funnel result");
    }

    Ok(Json(response))
}

pub(crate) fn build_steps(step_names: &[String], rows: &[FunnelRow]) -> Vec<FunnelStep> {
    let total: u64 = rows
        .iter()
        .filter(|r| r.level > 0)
        .map(|r| r.users_reached)
        .sum();

    let mut steps = Vec::with_capacity(step_names.len());
    let mut prev_users = total;

    for (i, name) in step_names.iter().enumerate() {
        let level = (i + 1) as u8;
        let users: u64 = rows
            .iter()
            .filter(|r| r.level >= level)
            .map(|r| r.users_reached)
            .sum();

        let conversion_rate = if prev_users > 0 {
            users as f64 / prev_users as f64
        } else {
            0.0
        };

        let overall_rate = if total > 0 {
            users as f64 / total as f64
        } else {
            0.0
        };

        steps.push(FunnelStep {
            name: name.clone(),
            users,
            conversion_rate: (conversion_rate * 1000.0).round() / 1000.0,
            overall_rate: (overall_rate * 1000.0).round() / 1000.0,
        });

        prev_users = users;
    }

    steps
}

#[cfg(test)]
mod tests {
    use crate::FunnelRow;

    use super::build_steps;

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    fn row(level: u8, users: u64) -> FunnelRow {
        FunnelRow {
            level,
            users_reached: users,
        }
    }

    #[test]
    fn build_steps_empty_rows() {
        let steps = build_steps(&names(&["Page Viewed"]), &[]);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].name, "Page Viewed");
        assert_eq!(steps[0].users, 0);
        assert_eq!(steps[0].conversion_rate, 0.0);
        assert_eq!(steps[0].overall_rate, 0.0);
    }

    #[test]
    fn build_steps_single_level() {
        let rows = vec![row(1, 100)];
        let steps = build_steps(&names(&["Page Viewed"]), &rows);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].users, 100);
        assert_eq!(steps[0].conversion_rate, 1.0);
        assert_eq!(steps[0].overall_rate, 1.0);
    }

    #[test]
    fn build_steps_cumulative_sum() {
        let rows = vec![row(1, 100), row(2, 50), row(3, 20)];
        let steps = build_steps(
            &names(&["Page Viewed", "Button Clicked", "Account Created"]),
            &rows,
        );
        assert_eq!(steps.len(), 3);

        assert_eq!(steps[0].name, "Page Viewed");
        assert_eq!(steps[0].users, 170);
        assert_eq!(steps[0].conversion_rate, 1.0);
        assert_eq!(steps[0].overall_rate, 1.0);

        assert_eq!(steps[1].name, "Button Clicked");
        assert_eq!(steps[1].users, 70);
        assert_eq!(steps[1].conversion_rate, 0.412);
        assert_eq!(steps[1].overall_rate, 0.412);

        assert_eq!(steps[2].name, "Account Created");
        assert_eq!(steps[2].users, 20);
        assert_eq!(steps[2].conversion_rate, 0.286);
        assert_eq!(steps[2].overall_rate, 0.118);
    }

    #[test]
    fn build_steps_level_zero_excluded_from_total() {
        let rows = vec![row(0, 999), row(1, 50), row(2, 25)];
        let steps = build_steps(&names(&["Step 1", "Step 2"]), &rows);
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].users, 75);
        assert_eq!(steps[1].users, 25);
    }

    #[test]
    fn build_steps_all_users_in_first_step() {
        let rows = vec![row(1, 100)];
        let steps = build_steps(&names(&["Step 1", "Step 2"]), &rows);
        assert_eq!(steps[0].users, 100);
        assert_eq!(steps[0].conversion_rate, 1.0);
        assert_eq!(steps[1].users, 0);
        assert_eq!(steps[1].conversion_rate, 0.0);
        assert_eq!(steps[1].overall_rate, 0.0);
    }

    #[test]
    fn build_steps_only_level_zero() {
        let rows = vec![row(0, 100)];
        let steps = build_steps(&names(&["Step 1"]), &rows);
        assert_eq!(steps[0].users, 0);
        assert_eq!(steps[0].conversion_rate, 0.0);
        assert_eq!(steps[0].overall_rate, 0.0);
    }

    #[test]
    fn build_steps_multiple_levels_same_count() {
        let rows = vec![row(1, 50), row(2, 50)];
        let steps = build_steps(&names(&["Step 1", "Step 2"]), &rows);
        assert_eq!(steps[0].users, 100);
        assert_eq!(steps[1].users, 50);
        assert_eq!(steps[1].conversion_rate, 0.5);
        assert_eq!(steps[1].overall_rate, 0.5);
    }
}
