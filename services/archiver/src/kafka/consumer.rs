use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use object_store::aws::AmazonS3Builder;
use object_store::ObjectStore;
use opsbucket_shared::events::RawEvent;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::{ClientConfig, Offset, TopicPartitionList};
use rdkafka::Message;
use tracing::{error, info, warn};

use crate::storage;

pub struct ArchiverConsumer {
    consumer: StreamConsumer,
    store: Arc<dyn ObjectStore>,
    s3_prefix: String,
    batch_size: usize,
    batch_timeout_ms: u64,
    max_file_size: usize,
    buffer: Vec<RawEvent>,
    running: bool,
}

impl ArchiverConsumer {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        brokers: &str,
        group_id: &str,
        s3_bucket: &str,
        s3_prefix: &str,
        s3_region: &str,
        s3_endpoint: Option<&str>,
        batch_size: usize,
        batch_timeout_ms: u64,
        max_file_size: usize,
    ) -> Result<Self> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .set("session.timeout.ms", "30000")
            .set("max.poll.interval.ms", "300000")
            .create()?;

        consumer.subscribe(&["raw-events"])?;

        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(s3_bucket)
            .with_region(s3_region);

        if let Some(endpoint) = s3_endpoint {
            builder = builder.with_endpoint(endpoint);
        }

        let store: Arc<dyn ObjectStore> = Arc::new(builder.build()?);

        Ok(Self {
            consumer,
            store,
            s3_prefix: s3_prefix.to_string(),
            batch_size,
            batch_timeout_ms,
            max_file_size,
            buffer: Vec::new(),
            running: true,
        })
    }

    pub fn shutdown(&mut self) {
        self.running = false;
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("archiver consumer started");

        loop {
            match self.process_batch().await {
                Ok(true) => {}
                Ok(false) => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(e) => {
                    error!(error = %e, "batch processing failed, retrying after backoff");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }

            if !self.running {
                break;
            }
        }

        self.flush().await?;
        info!("archiver consumer stopped");
        Ok(())
    }

    async fn process_batch(&mut self) -> Result<bool> {
        let mut partition_offsets: HashMap<i32, i64> = HashMap::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.batch_timeout_ms);

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() && !self.buffer.is_empty() {
                break;
            }
            if self.buffer.len() >= self.batch_size {
                break;
            }

            let msg = tokio::time::timeout(remaining, self.consumer.recv()).await;

            match msg {
                Ok(Ok(kafka_msg)) => {
                    partition_offsets.insert(kafka_msg.partition(), kafka_msg.offset());
                    if let Some(payload) = kafka_msg.payload() {
                        match serde_json::from_slice::<RawEvent>(payload) {
                            Ok(event) => self.buffer.push(event),
                            Err(e) => {
                                warn!(error = %e, "failed to deserialize Kafka message, skipping");
                            }
                        }
                    }
                }
                Ok(Err(_)) => {
                    if self.buffer.is_empty() {
                        return Ok(false);
                    }
                    break;
                }
                Err(_) => {
                    break;
                }
            }
        }

        if self.buffer.is_empty() {
            return Ok(false);
        }

        if self.buffer.len() >= self.batch_size || self.estimate_buffer_size() >= self.max_file_size
        {
            self.flush().await?;
        }

        if !partition_offsets.is_empty() {
            let mut tpl = TopicPartitionList::new();
            for (partition, offset) in &partition_offsets {
                tpl.add_partition_offset("raw-events", *partition, Offset::Offset(*offset + 1))?;
            }
            info!(
                offsets = ?partition_offsets,
                "committing offsets after successful archive write"
            );
            self.consumer.commit(&tpl, CommitMode::Sync)?;
        }

        Ok(true)
    }

    fn estimate_buffer_size(&self) -> usize {
        self.buffer.len() * 1024
    }

    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let events = std::mem::take(&mut self.buffer);
        let count = events.len();
        info!(count, "flushing events to S3 archive");

        let now = Utc::now();
        let dt = now.format("%Y-%m-%d").to_string();
        let ts = now.format("%Y%m%d%H%M%S").to_string();

        let parquet_bytes = storage::write_events_to_parquet(&events)?;

        let key = format!("{}/dt={}/part-{}.parquet", self.s3_prefix, dt, ts);
        let path = object_store::path::Path::from(key.clone());

        self.store.put(&path, parquet_bytes.into()).await?;
        info!(key = %key, count, "archived events to S3");

        Ok(())
    }
}
