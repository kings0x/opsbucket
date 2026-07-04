use std::collections::HashMap;
use std::time::Duration;

use anyhow::Result;
use clickhouse::Client as ChClient;
use opsbucket_shared::events::RawEvent;
use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::Message;
use rdkafka::{ClientConfig, Offset, TopicPartitionList};
use redis::aio::ConnectionManager as RedisConnectionManager;
use sqlx::PgPool;
use tracing::{error, info, warn};

use crate::clickhouse::client as ch_client;
use crate::dedup;
use crate::identity;
use crate::kafka::dlq::DlqProducer;
use crate::schema::flatten;
use crate::timestamp;

pub struct ProcessingConsumer {
    consumer: StreamConsumer,
    redis: RedisConnectionManager,
    pg: PgPool,
    clickhouse: ChClient,
    dlq: DlqProducer,
    batch_size: usize,
    batch_timeout_ms: u64,
    dedup_ttl: u64,
    alias_cache_ttl: u64,
    dlq_max_retries: u32,
    running: bool,
}

impl ProcessingConsumer {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        brokers: &str,
        group_id: &str,
        redis_url: &str,
        database_url: &str,
        clickhouse_url: &str,
        clickhouse_user: &str,
        clickhouse_password: &str,
        batch_size: usize,
        batch_timeout_ms: u64,
        dedup_ttl: u64,
        alias_cache_ttl: u64,
        dlq_max_retries: u32,
    ) -> Result<Self> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("group.id", group_id)
            .set("bootstrap.servers", brokers)
            .set("auto.offset.reset", "earliest")
            .set("enable.auto.commit", "false")
            .set("session.timeout.ms", "30000")
            .set("max.poll.interval.ms", "300000")
            .set("allow.auto.create.topics", "true")
            .create()?;

        consumer.subscribe(&["raw-events"])?;

        let redis_client = redis::Client::open(redis_url)?;
        let redis = RedisConnectionManager::new(redis_client).await?;

        let pg = PgPool::connect(database_url).await?;

        let clickhouse = ChClient::default()
            .with_url(clickhouse_url)
            .with_user(clickhouse_user)
            .with_password(clickhouse_password);

        let dlq = DlqProducer::new(brokers)?;

        Ok(Self {
            consumer,
            redis,
            pg,
            clickhouse,
            dlq,
            batch_size,
            batch_timeout_ms,
            dedup_ttl,
            alias_cache_ttl,
            dlq_max_retries,
            running: true,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        info!("processing consumer started");

        while self.running {
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

        info!("processing consumer stopped");
        Ok(())
    }

    pub fn shutdown(&mut self) {
        self.running = false;
    }

    async fn process_batch(&mut self) -> Result<bool> {
        let mut messages = Vec::new();
        let mut partition_offsets: HashMap<i32, i64> = HashMap::new();
        let deadline = tokio::time::Instant::now() + Duration::from_millis(self.batch_timeout_ms);

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() && !messages.is_empty() {
                break;
            }

            let msg = tokio::time::timeout(remaining, self.consumer.recv()).await;

            match msg {
                Ok(Ok(kafka_msg)) => {
                    partition_offsets.insert(kafka_msg.partition(), kafka_msg.offset());
                    if let Some(payload) = kafka_msg.payload() {
                        match serde_json::from_slice::<RawEvent>(payload) {
                            Ok(event) => {
                                messages.push(event);
                                if messages.len() >= self.batch_size {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!(error = %e, "failed to deserialize Kafka message");
                                self.send_to_dlq(
                                    &serde_json::from_slice(payload).unwrap_or_default(),
                                    &e.to_string(),
                                    "deserialization",
                                )
                                .await;
                            }
                        }
                    }
                }
                Ok(Err(_)) => {
                    if messages.is_empty() {
                        return Ok(false);
                    }
                    break;
                }
                Err(_) => {
                    break;
                }
            }
        }

        if messages.is_empty() {
            if !partition_offsets.is_empty() {
                let mut tpl = TopicPartitionList::new();
                for (partition, offset) in &partition_offsets {
                    tpl.add_partition_offset("raw-events", *partition, Offset::Offset(*offset + 1))?;
                }
                info!(
                    offsets = ?partition_offsets,
                    "committing offsets after DLQ-only batch"
                );
                self.consumer.commit(&tpl, CommitMode::Sync)?;
            }
            return Ok(false);
        }

        self.process_events(messages).await?;

        if !partition_offsets.is_empty() {
            let mut tpl = TopicPartitionList::new();
            for (partition, offset) in &partition_offsets {
                tpl.add_partition_offset("raw-events", *partition, Offset::Offset(*offset + 1))?;
            }
            info!(
                offsets = ?partition_offsets,
                "committing offsets after successful batch"
            );
            self.consumer.commit(&tpl, CommitMode::Sync)?;
        }

        Ok(true)
    }

    async fn process_events(&mut self, events: Vec<RawEvent>) -> Result<()> {
        let deduped = dedup::filter(events, &mut self.redis, self.dedup_ttl).await?;
        if deduped.is_empty() {
            return Ok(());
        }

        let (identify_events, mut non_identify): (Vec<RawEvent>, Vec<RawEvent>) = deduped
            .into_iter()
            .partition(|e| e.event_type == "identify");

        for event in &identify_events {
            if let Err(e) =
                identity::handlers::handle_identify(event, &self.pg, &mut self.redis).await
            {
                if self.should_dlq(&event.message_id, "identify").await {
                    warn!(
                        message_id = %event.message_id,
                        error = %e,
                        "identify handler permanently failed, routing to DLQ"
                    );
                    self.send_to_dlq(
                        &serde_json::to_value(event).unwrap_or_default(),
                        &e.to_string(),
                        "identify",
                    )
                    .await;
                }
            } else {
                non_identify.push(event.clone());
            }
        }

        let resolved = identity::resolver::resolve(
            non_identify,
            &mut self.redis,
            &self.pg,
            self.alias_cache_ttl,
        )
        .await?;

        let timestamped = timestamp::correct(resolved)?;

        let rows: Vec<_> = timestamped.into_iter().map(flatten::flatten).collect();

        ch_client::insert_batch(&self.clickhouse, rows).await?;

        Ok(())
    }

    async fn should_dlq(&mut self, message_id: &str, stage: &str) -> bool {
        let key = format!("retry:{}:{}", stage, message_id);
        let count: u32 = redis::cmd("INCR")
            .arg(&key)
            .query_async(&mut self.redis)
            .await
            .unwrap_or(1);
        if count == 1 {
            let _: () = redis::cmd("EXPIRE")
                .arg(&key)
                .arg(3600i64)
                .query_async(&mut self.redis)
                .await
                .unwrap_or_default();
        }
        count > self.dlq_max_retries
    }

    async fn send_to_dlq(&self, event: &serde_json::Value, reason: &str, stage: &str) {
        if let Err(e) = self.dlq.send(event, reason, stage).await {
            error!(error = %e, "DLQ send failed");
        }
    }
}
