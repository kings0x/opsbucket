use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub kafka_consumer_group: String,
    pub s3_bucket: String,
    pub s3_prefix: String,
    pub s3_region: String,
    pub s3_endpoint: Option<String>,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub max_file_size: usize,
    pub rust_log: String,
}

impl Config {
    pub fn load() -> Self {
        Self {
            kafka_brokers: env::var("KAFKA_BROKERS").expect("KAFKA_BROKERS must be set"),
            kafka_consumer_group: env::var("KAFKA_CONSUMER_GROUP")
                .unwrap_or_else(|_| "opsbucket-archiver".to_string()),
            s3_bucket: env::var("ARCHIVE_S3_BUCKET").expect("ARCHIVE_S3_BUCKET must be set"),
            s3_prefix: env::var("ARCHIVE_S3_PREFIX").unwrap_or_else(|_| "raw-events".to_string()),
            s3_region: env::var("ARCHIVE_S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            s3_endpoint: env::var("ARCHIVE_S3_ENDPOINT").ok(),
            batch_size: env::var("BATCH_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(10000),
            batch_timeout_ms: env::var("BATCH_TIMEOUT_MS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(60000),
            max_file_size: env::var("ARCHIVE_MAX_FILE_SIZE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(67108864),
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}
