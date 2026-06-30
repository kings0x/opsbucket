use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;

use opsbucket_processing::config::Config;
use opsbucket_processing::kafka::consumer::ProcessingConsumer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    tracing_subscriber::fmt()
        .with_env_filter(&config.rust_log)
        .init();

    info!("starting opsbucket-processing");

    let mut consumer = ProcessingConsumer::new(
        &config.kafka_brokers,
        &config.kafka_consumer_group,
        &config.redis_url,
        &config.database_url,
        &config.clickhouse_url,
        config.batch_size,
        config.batch_timeout_ms,
        config.dedup_ttl_seconds,
        config.alias_cache_ttl_seconds,
        config.dlq_max_retries,
    )
    .await?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        info!("shutdown signal received");
        r.store(false, Ordering::SeqCst);
    });

    consumer.run().await?;

    Ok(())
}
