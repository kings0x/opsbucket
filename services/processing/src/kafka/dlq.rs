use std::time::Duration;

use anyhow::Result;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct DlqPayload {
    event: serde_json::Value,
    failure_reason: String,
    failed_stage: String,
}

pub struct DlqProducer {
    producer: FutureProducer,
    topic: String,
}

impl DlqProducer {
    pub fn new(brokers: &str) -> Result<Self> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .set("allow.auto.create.topics", "false")
            .create()?;
        Ok(Self {
            producer,
            topic: "raw-events-dlq".to_string(),
        })
    }

    pub async fn send(
        &self,
        event: &serde_json::Value,
        failure_reason: &str,
        failed_stage: &str,
    ) -> Result<()> {
        let payload = DlqPayload {
            event: event.clone(),
            failure_reason: failure_reason.to_string(),
            failed_stage: failed_stage.to_string(),
        };
        let body = serde_json::to_vec(&payload)?;
        let record = FutureRecord::to(&self.topic)
            .payload(&body)
            .key(failed_stage);
        self.producer
            .send(record, Duration::from_secs(5))
            .await
            .map_err(|(e, _)| anyhow::anyhow!("DLQ send failed: {}", e))?;
        Ok(())
    }
}
