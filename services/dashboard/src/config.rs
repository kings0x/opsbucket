#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub query_url: String,
    pub query_secret_key: String,
    pub port: u16,
    pub rust_log: String,
    pub session_ttl_seconds: u64,
}

impl Config {
    pub fn load() -> Self {
        Config {
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            redis_url: std::env::var("REDIS_URL").expect("REDIS_URL must be set"),
            query_url: std::env::var("QUERY_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8081".into()),
            query_secret_key: std::env::var("SECRET_KEY").expect("SECRET_KEY must be set"),
            port: std::env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8082),
            rust_log: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
            session_ttl_seconds: std::env::var("SESSION_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86400),
        }
    }
}
