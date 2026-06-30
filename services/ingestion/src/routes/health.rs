use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

use crate::AppState;

pub async fn get_health(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let pg_ok = state.pg.check_health().await.is_ok();
    let redis_ok = state.redis.check_health().await.is_ok();
    let kafka_ok = state.kafka_health.check_health().await.is_ok();

    let all_ok = pg_ok && redis_ok && kafka_ok;

    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    (
        status_code,
        Json(json!({
            "status": if all_ok { "ok" } else { "degraded" },
            "checks": {
                "postgres": if pg_ok { "ok" } else { "unhealthy" },
                "redis": if redis_ok { "ok" } else { "unhealthy" },
                "kafka": if kafka_ok { "ok" } else { "unhealthy" },
            }
        })),
    )
}
