pub mod auth;
pub mod kafka;
pub mod rate_limiter;
pub mod routes;
pub mod validation;

use std::sync::Mutex;

pub struct AppState {
    pub redis: redis::Client,
    pub pg: sqlx::PgPool,
    pub kafka: kafka::producer::KafkaProducer,
    pub rate_limiter: Mutex<rate_limiter::RateLimiter>,
}
