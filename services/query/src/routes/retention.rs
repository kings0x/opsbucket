use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Json;

use crate::auth::secret_key::check_auth;
use crate::cache;
use crate::ch::client;
use crate::queries::{builder, retention};
use crate::{
    AppError, AppState, Cohort, CohortPeriod, RetentionResponse, RetentionRow, RetentionSpec,
};

pub async fn handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<RetentionResponse>, AppError> {
    check_auth(&headers, &state.secret_key).map_err(AppError::unauthorized)?;
    let spec: RetentionSpec = serde_json::from_slice(&body)
        .map_err(|_| AppError::invalid_request("request body is empty or malformed".into()))?;
    builder::validate_retention_with_max_date_range(&spec, state.config.max_date_range_days)
        .map_err(AppError::invalid_request)?;

    let cache_key = cache::cache_key(&spec);
    let mut redis = state.redis.clone();
    if let Some(mut cached) = cache::get::<RetentionResponse>(&mut redis, &cache_key)
        .await
        .map_err(AppError::from_anyhow)?
    {
        cached.cached_at = Some(chrono::Utc::now().to_rfc3339());
        return Ok(Json(cached));
    }

    let plan = retention::build_plan(&spec);
    let rows: Vec<RetentionRow> =
        client::query_plan(&state.ch_client, &plan, state.config.query_timeout_seconds)
            .await
            .map_err(AppError::from)?;

    let cohorts = build_cohorts(&rows, spec.periods);
    let response = RetentionResponse {
        cohorts,
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
        tracing::warn!(error = %e, "failed to cache retention result");
    }

    Ok(Json(response))
}

fn build_cohorts(rows: &[RetentionRow], requested_periods: u64) -> Vec<Cohort> {
    use std::collections::BTreeMap;

    let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
    let mut map: BTreeMap<chrono::NaiveDate, Vec<&RetentionRow>> = BTreeMap::new();
    for row in rows {
        let cohort_date = epoch
            .checked_add_days(chrono::Days::new(row.cohort_date as u64))
            .unwrap();
        map.entry(cohort_date).or_default().push(row);
    }

    map.into_iter()
        .map(|(cohort_date, cohort_rows)| {
            let initial_users = cohort_rows
                .iter()
                .find(|r| r.period == 0)
                .map(|r| r.users)
                .unwrap_or(0);

            let mut periods = Vec::new();
            for p in 0..requested_periods {
                let users = cohort_rows
                    .iter()
                    .find(|r| r.period == p as i32)
                    .map(|r| r.users)
                    .unwrap_or(0);
                let rate = if initial_users > 0 {
                    users as f64 / initial_users as f64
                } else {
                    0.0
                };
                periods.push(CohortPeriod {
                    period: p,
                    users,
                    rate: (rate * 1000.0).round() / 1000.0,
                });
            }

            Cohort {
                cohort_date: cohort_date.format("%Y-%m-%d").to_string(),
                initial_users,
                periods,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::RetentionRow;

    use super::build_cohorts;

    fn row(cohort_date: i32, period: i32, users: u64) -> RetentionRow {
        RetentionRow {
            cohort_date,
            period,
            users,
        }
    }

    #[test]
    fn build_cohorts_empty_rows() {
        let cohorts = build_cohorts(&[], 8);
        assert!(cohorts.is_empty());
    }

    #[test]
    fn build_cohorts_single_cohort_period_0() {
        let rows = vec![row(19000, 0, 100)];
        let cohorts = build_cohorts(&rows, 8);
        assert_eq!(cohorts.len(), 1);
        assert_eq!(cohorts[0].initial_users, 100);
        assert_eq!(cohorts[0].periods.len(), 8);
        assert_eq!(cohorts[0].periods[0].users, 100);
        assert_eq!(cohorts[0].periods[0].rate, 1.0);
    }

    #[test]
    fn build_cohorts_single_cohort_with_retention() {
        let rows = vec![row(19000, 0, 100), row(19000, 1, 60), row(19000, 2, 30)];
        let cohorts = build_cohorts(&rows, 8);
        assert_eq!(cohorts.len(), 1);
        assert_eq!(cohorts[0].initial_users, 100);
        assert_eq!(cohorts[0].periods[0].users, 100);
        assert_eq!(cohorts[0].periods[0].rate, 1.0);
        assert_eq!(cohorts[0].periods[1].users, 60);
        assert_eq!(cohorts[0].periods[1].rate, 0.6);
        assert_eq!(cohorts[0].periods[2].users, 30);
        assert_eq!(cohorts[0].periods[2].rate, 0.3);
    }

    #[test]
    fn build_cohorts_multiple_cohorts() {
        let rows = vec![
            row(19000, 0, 100),
            row(19000, 1, 40),
            row(19001, 0, 200),
            row(19001, 1, 80),
        ];
        let cohorts = build_cohorts(&rows, 4);
        assert_eq!(cohorts.len(), 2);

        assert_eq!(cohorts[0].initial_users, 100);
        assert_eq!(cohorts[0].periods[0].users, 100);
        assert_eq!(cohorts[0].periods[1].users, 40);

        assert_eq!(cohorts[1].initial_users, 200);
        assert_eq!(cohorts[1].periods[0].users, 200);
        assert_eq!(cohorts[1].periods[1].users, 80);
    }

    #[test]
    fn build_cohorts_missing_periods_return_zero() {
        let rows = vec![row(19000, 0, 100)];
        let cohorts = build_cohorts(&rows, 4);
        assert_eq!(cohorts[0].periods.len(), 4);
        assert_eq!(cohorts[0].periods[0].users, 100);
        assert_eq!(cohorts[0].periods[1].users, 0);
        assert_eq!(cohorts[0].periods[1].rate, 0.0);
        assert_eq!(cohorts[0].periods[2].users, 0);
        assert_eq!(cohorts[0].periods[3].users, 0);
    }

    #[test]
    fn build_cohorts_rate_precision_three_decimals() {
        let rows = vec![row(19000, 0, 100), row(19000, 1, 33)];
        let cohorts = build_cohorts(&rows, 4);
        assert_eq!(cohorts[0].periods[1].rate, 0.33);
    }

    #[test]
    fn build_cohorts_zero_initial_users_returns_zero_rate() {
        let rows = vec![row(19000, 0, 0), row(19000, 1, 50)];
        let cohorts = build_cohorts(&rows, 4);
        assert_eq!(cohorts[0].initial_users, 0);
        assert_eq!(cohorts[0].periods[1].rate, 0.0);
    }

    #[test]
    fn build_cohorts_cohort_date_formats_as_date_string() {
        let rows = vec![row(19000, 0, 100)];
        let cohorts = build_cohorts(&rows, 1);
        assert_eq!(cohorts[0].cohort_date.len(), 10);
    }

    #[test]
    fn build_cohorts_requested_periods_determines_output_length() {
        let rows = vec![row(19000, 0, 100)];
        let cohorts_4 = build_cohorts(&rows, 4);
        let cohorts_12 = build_cohorts(&rows, 12);
        assert_eq!(cohorts_4[0].periods.len(), 4);
        assert_eq!(cohorts_12[0].periods.len(), 12);
    }
}
