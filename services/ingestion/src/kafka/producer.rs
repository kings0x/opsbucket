use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use opsbucket_shared::events::RawEvent;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;

use super::KafkaHealth;

#[async_trait]
pub trait EventProducer: Send + Sync {
    async fn send_raw(&self, project_id: &str, events: &[RawEvent]) -> Result<()>;
}

pub struct KafkaProducer {
    producer: FutureProducer,
}

impl KafkaProducer {
    pub fn new(brokers: &str) -> Result<Self> {
        let producer: FutureProducer = rdkafka::config::ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", "1")
            .set("queue.buffering.max.ms", "50")
            .set("batch.num.messages", "100")
            .set("message.timeout.ms", "5000")
            .set("compression.type", "none")
            .set("max.in.flight", "5")
            .create()?;
        Ok(Self { producer })
    }
}

#[async_trait]
impl EventProducer for KafkaProducer {
    async fn send_raw(&self, project_id: &str, events: &[RawEvent]) -> Result<()> {
        for event in events {
            let payload = serde_json::to_vec(event)?;
            let record = FutureRecord::to("raw-events")
                .key(project_id)
                .payload(&payload);
            match self.producer.send(record, Duration::from_secs(5)).await {
                Ok(_) => {}
                Err(e) => return Err(anyhow::anyhow!("kafka: {:#?}", e)),
            }
        }
        Ok(())
    }
}

#[async_trait]
impl KafkaHealth for KafkaProducer {
    async fn check_health(&self) -> Result<()> {
        self.producer
            .client()
            .fetch_metadata(None, Timeout::After(Duration::from_secs(5)))?;
        Ok(())
    }
}

pub struct MockProducer {
    pub messages: std::sync::Mutex<Vec<(String, Vec<RawEvent>)>>,
    pub fail_on_send: std::sync::Mutex<bool>,
    pub fail_health: std::sync::Mutex<bool>,
}

impl Default for MockProducer {
    fn default() -> Self {
        Self {
            messages: std::sync::Mutex::new(Vec::new()),
            fail_on_send: std::sync::Mutex::new(false),
            fail_health: std::sync::Mutex::new(false),
        }
    }
}

impl MockProducer {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl EventProducer for MockProducer {
    async fn send_raw(&self, project_id: &str, events: &[RawEvent]) -> Result<()> {
        if *self.fail_on_send.lock().unwrap() {
            return Err(anyhow::anyhow!("mock: kafka unavailable"));
        }
        self.messages
            .lock()
            .unwrap()
            .push((project_id.to_string(), events.to_vec()));
        Ok(())
    }
}

#[async_trait]
impl KafkaHealth for MockProducer {
    async fn check_health(&self) -> Result<()> {
        if *self.fail_health.lock().unwrap() {
            Err(anyhow::anyhow!("mock: kafka unhealthy"))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsbucket_shared::events::{Campaign, Context, Library, Page, Screen};

    fn make_raw_event(project_id: &str, message_id: &str) -> RawEvent {
        RawEvent {
            project_id: project_id.to_string(),
            received_at: "2026-06-29T10:00:00.000Z".to_string(),
            sent_at: "2026-06-29T10:00:00.000Z".to_string(),
            ip: "127.0.0.1".to_string(),
            message_id: message_id.to_string(),
            event_type: "track".to_string(),
            anonymous_id: "anon-1".to_string(),
            user_id: None,
            original_timestamp: "2026-06-29T10:00:00.000Z".to_string(),
            context: Context {
                library: Library {
                    name: "test".into(),
                    version: "1.0".into(),
                },
                page: Page {
                    url: "https://example.com".into(),
                    path: "/".into(),
                    referrer: "".into(),
                    title: "Test".into(),
                    search: "".into(),
                },
                screen: Screen {
                    width: 1920,
                    height: 1080,
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
                ip: Some("127.0.0.1".into()),
            },
            event: Some("test event".into()),
            properties: Some(serde_json::json!({"key": "value"})),
            traits: None,
            name: None,
        }
    }

    #[tokio::test]
    async fn sends_events_to_mock() {
        let producer = MockProducer::new();
        let events = vec![
            make_raw_event("proj-1", "msg-1"),
            make_raw_event("proj-1", "msg-2"),
        ];

        producer.send_raw("proj-1", &events).await.unwrap();

        let stored = producer.messages.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].0, "proj-1");
        assert_eq!(stored[0].1.len(), 2);
    }

    #[tokio::test]
    async fn error_on_kafka_failure() {
        let producer = MockProducer::new();
        *producer.fail_on_send.lock().unwrap() = true;
        let events = vec![make_raw_event("proj-1", "msg-1")];

        let result = producer.send_raw("proj-1", &events).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("kafka unavailable"));
    }

    #[tokio::test]
    async fn empty_batch_is_ok() {
        let producer = MockProducer::new();
        let events = vec![];

        producer.send_raw("proj-1", &events).await.unwrap();

        let stored = producer.messages.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert!(stored[0].1.is_empty());
    }

    #[tokio::test]
    async fn multiple_projects_are_independent() {
        let producer = MockProducer::new();

        producer
            .send_raw("proj-a", &[make_raw_event("proj-a", "msg-1")])
            .await
            .unwrap();
        producer
            .send_raw("proj-b", &[make_raw_event("proj-b", "msg-2")])
            .await
            .unwrap();

        let stored = producer.messages.lock().unwrap();
        assert_eq!(stored.len(), 2);
        assert_eq!(stored[0].0, "proj-a");
        assert_eq!(stored[1].0, "proj-b");
    }
}
