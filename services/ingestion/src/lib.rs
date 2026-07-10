pub mod auth;
pub mod config;
pub mod db;
pub mod kafka;
pub mod rate_limiter;
pub mod routes;
pub mod server;
pub mod validation;

use std::sync::{Arc, Mutex};

use auth::AuthValidator;
use db::{PgHealth, RedisHealth};
use kafka::{producer::EventProducer, KafkaHealth};
use rate_limiter::RateLimiter;

pub struct AppState {
    pub auth: Arc<dyn AuthValidator>,
    pub kafka: Arc<dyn EventProducer>,
    pub kafka_health: Arc<dyn KafkaHealth>,
    pub pg: Arc<dyn PgHealth>,
    pub redis: Arc<dyn RedisHealth>,
    pub rate_limiter: Mutex<RateLimiter>,
    pub replay_rate_limiter: Mutex<RateLimiter>,
    pub trust_proxy_headers: bool,
}
