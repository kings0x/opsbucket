use std::time::Duration;

use anyhow::Result;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::ClientConfig;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct DlqPayload {
    raw: String,
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

    pub async fn send_raw(
        &self,
        raw: &str,
        failure_reason: &str,
        failed_stage: &str,
    ) -> Result<()> {
        let payload = DlqPayload {
            raw: raw.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dlq_payload_serializes_to_expected_json() {
        let payload = DlqPayload {
            raw: "{\"event\":\"test\"}".into(),
            failure_reason: "invalid timestamp".into(),
            failed_stage: "deserialization".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["raw"], "{\"event\":\"test\"}");
        assert_eq!(json["failure_reason"], "invalid timestamp");
        assert_eq!(json["failed_stage"], "deserialization");
    }

    #[test]
    fn dlq_payload_handles_empty_fields() {
        let payload = DlqPayload {
            raw: String::new(),
            failure_reason: String::new(),
            failed_stage: String::new(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["raw"], "");
        assert_eq!(json["failure_reason"], "");
        assert_eq!(json["failed_stage"], "");
    }

    #[test]
    fn dlq_payload_serializes_unicode_content() {
        let payload = DlqPayload {
            raw: "{\"name\":\"café\"}".into(),
            failure_reason: "événement invalide".into(),
            failed_stage: "parsing".into(),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["raw"], "{\"name\":\"café\"}");
        assert_eq!(json["failure_reason"], "événement invalide");
    }
}
