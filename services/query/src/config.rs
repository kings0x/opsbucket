#[derive(Clone)]
pub struct Config {
    pub secret_key: String,
    pub database_url: String,
    pub redis_url: String,
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub query_cache_ttl_seconds: u64,
    pub query_timeout_seconds: u64,
    pub max_date_range_days: u64,
    pub s3_endpoint: String,
    pub cors_allowed_origins: Vec<String>,
    pub port: u16,
    pub rust_log: String,
}

impl Config {
    pub fn load() -> Self {
        Self {
            secret_key: std::env::var("SECRET_KEY").expect("SECRET_KEY must be set"),
            database_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            redis_url: std::env::var("REDIS_URL").expect("REDIS_URL must be set"),
            clickhouse_url: std::env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL must be set"),
            clickhouse_user: std::env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".into()),
            clickhouse_password: std::env::var("CLICKHOUSE_PASSWORD").unwrap_or_default(),
            query_cache_ttl_seconds: std::env::var("QUERY_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60),
            query_timeout_seconds: std::env::var("QUERY_TIMEOUT_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            max_date_range_days: std::env::var("MAX_DATE_RANGE_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(366),
            s3_endpoint: std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT must be set"),
            cors_allowed_origins: std::env::var("CORS_ALLOWED_ORIGINS")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(str::to_string)
                .collect(),
            port: std::env::var("PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(8080),
            rust_log: std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        }
    }
}
