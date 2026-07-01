use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_consumer_group: String,
    pub database_url: String,
    pub redis_url: String,
    pub clickhouse_url: String,
    pub clickhouse_user: String,
    pub clickhouse_password: String,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub dedup_ttl_seconds: u64,
    pub alias_cache_ttl_seconds: u64,
    pub dlq_max_retries: u32,
    pub rust_log: String,
}

impl Config {
    pub fn load() -> Self {
        Self {
            kafka_brokers: env::var("KAFKA_BROKERS").expect("KAFKA_BROKERS must be set"),
            kafka_consumer_group: env::var("KAFKA_CONSUMER_GROUP")
                .unwrap_or_else(|_| "opsbucket-processing".to_string()),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            redis_url: env::var("REDIS_URL").expect("REDIS_URL must be set"),
            clickhouse_url: env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL must be set"),
            clickhouse_user: env::var("CLICKHOUSE_USER").unwrap_or_else(|_| "default".to_string()),
            clickhouse_password: env::var("CLICKHOUSE_PASSWORD").unwrap_or_else(|_| "".to_string()),
            batch_size: env::var("BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1000),
            batch_timeout_ms: env::var("BATCH_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(5000),
            dedup_ttl_seconds: env::var("DEDUP_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(86400),
            alias_cache_ttl_seconds: env::var("ALIAS_CACHE_TTL_SECONDS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(600),
            dlq_max_retries: env::var("DLQ_MAX_RETRIES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(3),
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}
