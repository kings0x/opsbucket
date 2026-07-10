use std::env;

#[derive(Debug, Clone)]
pub struct Config {
    pub kafka_brokers: String,
    pub database_url: String,
    pub s3_endpoint: String,
    pub s3_bucket: String,
    pub s3_region: String,
    pub s3_access_key: String,
    pub s3_secret_key: String,
    pub rust_log: String,
}

impl Config {
    pub fn load() -> Self {
        Self {
            kafka_brokers: env::var("KAFKA_BROKERS").expect("KAFKA_BROKERS must be set"),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            s3_endpoint: env::var("S3_ENDPOINT").expect("S3_ENDPOINT must be set"),
            s3_bucket: env::var("S3_BUCKET").unwrap_or_else(|_| "opsbucket-replay".to_string()),
            s3_region: env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".to_string()),
            s3_access_key: env::var("S3_ACCESS_KEY").expect("S3_ACCESS_KEY must be set"),
            s3_secret_key: env::var("S3_SECRET_KEY").expect("S3_SECRET_KEY must be set"),
            rust_log: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        }
    }
}
