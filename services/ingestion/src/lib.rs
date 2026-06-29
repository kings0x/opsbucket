pub mod auth;
pub mod kafka;
pub mod rate_limiter;
pub mod routes;
pub mod validation;

use std::sync::{Arc, Mutex};

use kafka::producer::EventProducer;

pub struct AppState {
    pub redis: redis::Client,
    pub pg: sqlx::PgPool,
    pub kafka: Arc<dyn EventProducer>,
    pub rate_limiter: Mutex<rate_limiter::RateLimiter>,
}
