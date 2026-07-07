use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use chrono::Utc;
use object_store::aws::AmazonS3Builder;
use object_store::ObjectStore;
use opsbucket_shared::events::RawEvent;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::Message;
use rdkafka::{ClientConfig, Offset, TopicPartitionList};
use tracing::{error, info, warn};

use crate::kafka::dlq::DlqProducer;
use crate::storage;

pub struct ArchiverConsumer {
    consumer: StreamConsumer,
    store: Arc<dyn ObjectStore>,
    dlq: DlqProducer,
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
        let dlq = DlqProducer::new(brokers)?;
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .set("session.timeout.ms", "30000")
            .set("max.poll.interval.ms", "300000")
            .create()?;

        consumer.subscribe(&["raw-events"])?;

        let access_key = std::env::var("AWS_ACCESS_KEY_ID")
            .or_else(|_| std::env::var("AWS_ACCESS_KEY"))
            .unwrap_or_default();
        let secret_key = std::env::var("AWS_SECRET_ACCESS_KEY")
            .or_else(|_| std::env::var("AWS_SECRET_KEY"))
            .unwrap_or_default();

        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(s3_bucket)
            .with_region(s3_region)
            .with_access_key_id(access_key)
            .with_secret_access_key(secret_key);

        if let Some(endpoint) = s3_endpoint {
            builder = builder
                .with_endpoint(endpoint)
                .with_virtual_hosted_style_request(false)
                .with_allow_http(true);
        }

        let store: Arc<dyn ObjectStore> = Arc::new(builder.build()?);

        Ok(Self {
            consumer,
            store,
            dlq,
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
        self.run_until(Arc::new(AtomicBool::new(true))).await
    }

    pub async fn run_until(&mut self, running: Arc<AtomicBool>) -> Result<()> {
        info!("archiver consumer started");

        while self.running && running.load(Ordering::SeqCst) {
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
            if self.estimate_buffer_size() >= self.max_file_size {
                break;
            }

            let msg = tokio::time::timeout(remaining, self.consumer.recv()).await;

            match msg {
                Ok(Ok(kafka_msg)) => {
                    let partition = kafka_msg.partition();
                    let offset = kafka_msg.offset();
                    if let Some(payload) = kafka_msg.payload() {
                        match serde_json::from_slice::<RawEvent>(payload) {
                            Ok(event) => {
                                self.buffer.push(event);
                                partition_offsets.insert(partition, offset);
                            }
                            Err(e) => {
                                let raw = String::from_utf8_lossy(payload).to_string();
                                match self
                                    .dlq
                                    .send_raw(&raw, &e.to_string(), "deserialization")
                                    .await
                                {
                                    Ok(()) => {
                                        warn!(%offset, "malformed event sent to DLQ");
                                        partition_offsets.insert(partition, offset);
                                    }
                                    Err(dlq_err) => {
                                        error!(error = %dlq_err, %offset, "DLQ send failed for malformed event, will retry");
                                    }
                                }
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

        self.flush().await?;

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

    async fn flush(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let mut by_date: BTreeMap<String, Vec<RawEvent>> = BTreeMap::new();
        for event in std::mem::take(&mut self.buffer) {
            let dt = archive_date(&event);
            by_date.entry(dt).or_default().push(event);
        }

        for (dt, events) in by_date {
            let count = events.len();
            info!(count, dt, "flushing events to S3 archive");

            let ts = Utc::now().timestamp_millis();
            let parquet_bytes = storage::write_events_to_parquet(&events)?;

            let key = archive_key(&self.s3_prefix, &dt, ts, count);
            let path = object_store::path::Path::from(key.clone());

            self.store.put(&path, parquet_bytes.into()).await?;
            info!(key = %key, count, "archived events to S3");
        }

        Ok(())
    }

    fn estimate_buffer_size(&self) -> usize {
        self.buffer.len() * 1024
    }
}

pub fn archive_key(prefix: &str, dt: &str, ts: i64, count: usize) -> String {
    format!("{prefix}/dt/{dt}/part-{ts}-{count}.parquet")
}

fn archive_date(event: &RawEvent) -> String {
    chrono::DateTime::parse_from_rfc3339(&event.original_timestamp)
        .or_else(|_| chrono::DateTime::parse_from_rfc3339(&event.received_at))
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| Utc::now().format("%Y-%m-%d").to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsbucket_shared::events::{Campaign, Context, Library, Page, Screen};

    fn make_event(original_timestamp: &str, received_at: &str) -> RawEvent {
        RawEvent {
            project_id: "proj_1".into(),
            received_at: received_at.to_string(),
            sent_at: "2026-06-30T12:00:00Z".into(),
            ip: "127.0.0.1".into(),
            message_id: "msg-1".into(),
            event_type: "track".into(),
            anonymous_id: "anon_1".into(),
            user_id: None,
            original_timestamp: original_timestamp.to_string(),
            context: Context {
                library: Library {
                    name: "test".into(),
                    version: "1.0".into(),
                },
                page: Page {
                    url: "".into(),
                    path: "".into(),
                    referrer: "".into(),
                    title: "".into(),
                    search: "".into(),
                },
                screen: Screen {
                    width: 0,
                    height: 0,
                    density: 1.0,
                },
                user_agent: "test".into(),
                locale: "en-US".into(),
                timezone: "UTC".into(),
                campaign: Campaign {
                    source: None,
                    medium: None,
                    name: None,
                    term: None,
                    content: None,
                },
                ip: None,
            },
            event: None,
            properties: None,
            traits: None,
            name: None,
        }
    }

    #[test]
    fn uses_original_timestamp_when_valid() {
        let event = make_event("2026-06-15T10:00:00Z", "2026-06-30T12:00:00Z");
        assert_eq!(archive_date(&event), "2026-06-15");
    }

    #[test]
    fn falls_back_to_received_at_when_original_timestamp_invalid() {
        let event = make_event("not-a-date", "2026-06-30T12:00:00Z");
        assert_eq!(archive_date(&event), "2026-06-30");
    }

    #[test]
    fn formats_date_as_yyyy_mm_dd() {
        let event = make_event("2026-01-01T00:00:00Z", "2026-06-30T12:00:00Z");
        let date = archive_date(&event);
        assert_eq!(date.len(), 10);
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }

    #[test]
    fn archive_key_includes_prefix_date_timestamp_and_count() {
        let key = archive_key("raw-events", "2026-07-04", 1712345678000, 42);
        assert_eq!(
            key,
            "raw-events/dt/2026-07-04/part-1712345678000-42.parquet"
        );
    }

    #[test]
    fn archive_key_supports_custom_prefix() {
        let key = archive_key("custom/archive", "2026-01-01", 1000, 5);
        assert_eq!(key, "custom/archive/dt/2026-01-01/part-1000-5.parquet");
    }

    #[test]
    fn archive_key_handles_single_event() {
        let key = archive_key("test", "2026-12-31", 999, 1);
        assert_eq!(key, "test/dt/2026-12-31/part-999-1.parquet");
    }
}
