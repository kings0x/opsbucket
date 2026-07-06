use std::sync::Arc;

use redis::aio::ConnectionManager;
use sqlx::PgPool;

pub mod config;
pub mod db;
pub mod routes;
pub mod server;
pub mod spa;

pub struct AppState {
    pub pg: PgPool,
    pub redis: ConnectionManager,
    pub query_url: String,
    pub query_secret_key: String,
    pub http_client: reqwest::Client,
    pub session_ttl_seconds: u64,
}

pub type SharedState = Arc<AppState>;
