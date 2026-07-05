use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub database_url: String,
    pub redis_url: String,
    pub port: u16,
    pub rate_limit_capacity: u64,
    pub rate_limit_refill: u64,
    pub trust_proxy_headers: bool,
    pub cors_allowed_origins: Vec<String>,
    pub rust_log: String,
}

impl Config {
    pub fn load() -> Self {
        let kafka_brokers = env::var("KAFKA_BROKERS").expect("KAFKA_BROKERS must be set");
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let redis_url = env::var("REDIS_URL").expect("REDIS_URL must be set");

        let port = env::var("PORT")
            .unwrap_or_else(|_| "8080".to_string())
            .parse()
            .expect("PORT must be a valid number");

        let rate_limit_capacity = env::var("RATE_LIMIT_CAPACITY")
            .unwrap_or_else(|_| "1000".to_string())
            .parse()
            .expect("RATE_LIMIT_CAPACITY must be a valid number");

        let rate_limit_refill = env::var("RATE_LIMIT_REFILL")
            .unwrap_or_else(|_| "1000".to_string())
            .parse()
            .expect("RATE_LIMIT_REFILL must be a valid number");

        let trust_proxy_headers = env::var("TRUST_PROXY_HEADERS")
            .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
            .unwrap_or(false);

        let cors_allowed_origins = env::var("CORS_ALLOWED_ORIGINS")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect();

        let rust_log = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

        Self {
            kafka_brokers,
            database_url,
            redis_url,
            port,
            rate_limit_capacity,
            rate_limit_refill,
            trust_proxy_headers,
            cors_allowed_origins,
            rust_log,
        }
    }
}
