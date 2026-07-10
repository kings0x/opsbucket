use std::env::{self, VarError};

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
            kafka_brokers: get_env("KAFKA_BROKERS", None),
            kafka_consumer_group: get_env("KAFKA_CONSUMER_GROUP", Some("opsbucket-processing")),
            database_url: get_env("DATABASE_URL", None),
            redis_url: get_env("REDIS_URL", None),
            clickhouse_url: get_env("CLICKHOUSE_URL", None),
            clickhouse_user: get_env("CLICKHOUSE_USER", Some("default")),
            clickhouse_password: get_env("CLICKHOUSE_PASSWORD", Some("")),
            batch_size: get_env("BATCH_SIZE", Some("1000")),
            batch_timeout_ms: get_env("BATCH_TIMEOUT_MS", Some("5000")),
            dedup_ttl_seconds: get_env("DEDUP_TTL_SECONDS", Some("86400")),
            alias_cache_ttl_seconds: get_env("ALIAS_CACHE_TTL_SECONDS", Some("600")),
            dlq_max_retries: get_env("DLQ_MAX_RETRIES", Some("3")),
            rust_log: get_env("RUST_LOG", Some("info")),
        }
    }
}

pub fn get_env<T>(name: &str, fallback: Option<&str>) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    let value = env_or_fallback(name, fallback);

    value
        .parse()
        .unwrap_or_else(|e| panic!("Invalid value for {name}: {e}"))
}

fn env_or_fallback(name: &str, fallback: Option<&str>) -> String {
    match env::var(name) {
        Ok(val) => val,
        Err(e) => match e {
            VarError::NotPresent => match fallback {
                Some(val) => val.to_string(),
                None => panic!("{} must be set", name),
            },

            _ => panic!("Error occured: {}", e),
        },
    }
}
