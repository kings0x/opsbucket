pub mod auth;
pub mod kafka;
pub mod rate_limiter;
pub mod routes;
pub mod validation;

use std::sync::{Arc, Mutex};

use auth::AuthValidator;
use kafka::producer::EventProducer;

pub struct AppState {
    pub auth: Arc<dyn AuthValidator>,
    pub kafka: Arc<dyn EventProducer>,
    pub rate_limiter: Mutex<rate_limiter::RateLimiter>,
}
