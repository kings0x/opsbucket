use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::signal;

use opsbucket_archiver::config::Config;
use opsbucket_archiver::kafka::consumer::ArchiverConsumer;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    tracing_subscriber::fmt()
        .with_env_filter(&config.rust_log)
        .init();

    info!("starting opsbucket-archiver");

    let mut consumer = ArchiverConsumer::new(
        &config.kafka_brokers,
        &config.kafka_consumer_group,
        &config.s3_bucket,
        &config.s3_prefix,
        &config.s3_region,
        config.s3_endpoint.as_deref(),
        config.batch_size,
        config.batch_timeout_ms,
        config.max_file_size,
    )
    .await?;

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    tokio::spawn(async move {
        signal::ctrl_c().await.ok();
        info!("shutdown signal received");
        r.store(false, Ordering::SeqCst);
    });

    consumer.run_until(running).await?;

    Ok(())
}
