use std::sync::Arc;

use opsbucket_retrace::config::Config;
use opsbucket_retrace::consumer::{KafkaReplayConsumer, ReplayConsumer};
use opsbucket_retrace::db::PostgresStore;
use opsbucket_retrace::s3::S3Store;
use opsbucket_retrace::ReplayStore;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.rust_log))
        .init();

    let s3 = Arc::new(
        S3Store::new(
            &config.s3_endpoint,
            &config.s3_region,
            &config.s3_access_key,
            &config.s3_secret_key,
            &config.s3_bucket,
        )
        .await?,
    );

    let pg_pool = sqlx::PgPool::connect(&config.database_url).await?;
    let db: Arc<dyn ReplayStore> = Arc::new(PostgresStore::new(pg_pool));

    let consumer = KafkaReplayConsumer::new(
        &config.kafka_brokers,
        "opsbucket-retrace",
        s3,
        db,
        config.s3_bucket.clone(),
    )?;

    consumer.subscribe("replay_events")?;
    consumer.run().await
}
