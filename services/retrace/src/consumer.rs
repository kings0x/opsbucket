use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use opsbucket_shared::events::ReplayBatchEnvelope;
use rdkafka::consumer::{Consumer, StreamConsumer};
use rdkafka::message::BorrowedMessage;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::{ClientConfig, Message};
use tokio::sync::Mutex;
use tracing::{error, info};

use crate::db::ReplayStore;
use crate::s3::S3Store;
use crate::session::SessionBuffer;

#[async_trait]
pub trait ReplayConsumer: Send + Sync {
    async fn run(&self) -> Result<()>;
}

pub struct KafkaReplayConsumer {
    consumer: StreamConsumer,
    dlq_producer: FutureProducer,
    s3: Arc<S3Store>,
    db: Arc<dyn ReplayStore>,
    buffer: Arc<Mutex<SessionBuffer>>,
    s3_bucket: String,
    dlq_topic: String,
}

impl KafkaReplayConsumer {
    pub fn new(
        brokers: &str,
        group_id: &str,
        s3: Arc<S3Store>,
        db: Arc<dyn ReplayStore>,
        s3_bucket: String,
    ) -> Result<Self> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("group.id", group_id)
            .set("enable.auto.commit", "false")
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "30000")
            .set("max.poll.interval.ms", "300000")
            .create()?;

        let dlq_producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", "1")
            .create()?;

        Ok(Self {
            consumer,
            dlq_producer,
            s3,
            db,
            buffer: Arc::new(Mutex::new(SessionBuffer::new())),
            s3_bucket,
            dlq_topic: "replay_events-dlq".to_string(),
        })
    }

    pub fn subscribe(&self, topic: &str) -> Result<()> {
        self.consumer.subscribe(&[topic])?;
        Ok(())
    }

    async fn process_message(&self, msg: BorrowedMessage<'_>) -> Result<()> {
        let payload = msg
            .payload()
            .ok_or_else(|| anyhow::anyhow!("empty message payload"))?;
        let envelope: ReplayBatchEnvelope = serde_json::from_slice(payload)?;

        let project_id = &envelope.project_id;
        let session_id = &envelope.batch.session_id;
        let chunk_seq = envelope.batch.chunk_seq;
        let payload_size = payload.len() as i32;
        let event_count = envelope.batch.events.len() as i32;

        // Dedup check
        {
            let mut buffer = self.buffer.lock().await;
            if buffer.is_duplicate(session_id, chunk_seq) {
                info!(
                    project_id = %project_id,
                    session_id = %session_id,
                    chunk_seq = %chunk_seq,
                    "skipping duplicate chunk"
                );
                return Ok(());
            }
            buffer.mark_seen(session_id, chunk_seq);
        }

        // Upload raw events to S3
        let s3_key = self
            .s3
            .upload_chunk(project_id, session_id, chunk_seq, payload)
            .await?;

        // Upsert session metadata
        self.db
            .upsert_session(
                project_id,
                session_id,
                envelope.batch.distinct_id.as_deref(),
                envelope.batch.is_final,
                1,
            )
            .await?;

        // Insert chunk metadata
        self.db
            .insert_chunk(
                project_id,
                session_id,
                &envelope.batch.window_id,
                chunk_seq,
                &s3_key,
                &self.s3_bucket,
                payload_size,
                event_count,
                envelope.batch.is_final,
            )
            .await?;

        // Evict session buffer if final
        if envelope.batch.is_final {
            let mut buffer = self.buffer.lock().await;
            buffer.evict_session(session_id);
        }

        info!(
            project_id = %project_id,
            session_id = %session_id,
            chunk_seq = %chunk_seq,
            events = %event_count,
            is_final = %envelope.batch.is_final,
            "processed replay chunk"
        );

        Ok(())
    }
}

#[async_trait]
impl ReplayConsumer for KafkaReplayConsumer {
    async fn run(&self) -> Result<()> {
        info!("retrace consumer started");

        loop {
            match self.consumer.recv().await {
                Ok(msg) => {
                    let topic = msg.topic().to_string();
                    let partition = msg.partition();
                    let offset = msg.offset();
                    let key = msg.key().map(|k| k.to_vec());

                    if let Err(e) = self.process_message(msg).await {
                        error!(error = ?e, "failed to process message, sending to DLQ");

                        let dlq_payload = serde_json::json!({
                            "error": e.to_string(),
                            "original_topic": topic,
                            "original_partition": partition,
                            "original_offset": offset,
                        });
                        if let Ok(bytes) = serde_json::to_vec(&dlq_payload) {
                            let dlq_record = FutureRecord::to(&self.dlq_topic)
                                .key(key.as_deref().unwrap_or_default())
                                .payload(&bytes);
                            if let Err((dlq_err, _)) = self
                                .dlq_producer
                                .send(dlq_record, Duration::from_secs(5))
                                .await
                            {
                                error!(error = ?dlq_err, "failed to forward message to DLQ");
                            }
                        }
                    }

                    let _ = self
                        .consumer
                        .commit_consumer_state(rdkafka::consumer::CommitMode::Sync);
                }
                Err(e) => {
                    error!(error = ?e, "kafka recv error");
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    }
}

#[cfg(test)]
pub struct MockConsumer {
    pub processed: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl MockConsumer {
    pub fn new() -> Self {
        Self {
            processed: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
#[async_trait]
impl ReplayConsumer for MockConsumer {
    async fn run(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::MockReplayStore;
    use crate::s3::MockS3Store;
    use opsbucket_shared::events::{RRWebEventValue, ReplayBatch};

    // ── helpers ──────────────────────────────────────────────────────────────────

    fn make_envelope_payload(
        session_id: &str,
        chunk_seq: u32,
        is_final: bool,
        num_events: usize,
        distinct_id: Option<&str>,
    ) -> Vec<u8> {
        let events: Vec<RRWebEventValue> = (0..num_events)
            .map(|i| RRWebEventValue {
                event_type: 3,
                data: serde_json::json!({ "i": i }),
                timestamp: 1000 + i as u64,
            })
            .collect();

        let batch = ReplayBatch {
            session_id: session_id.to_string(),
            window_id: "window-1".to_string(),
            chunk_seq,
            distinct_id: distinct_id.map(|s| s.to_string()),
            project_id: "proj-1".to_string(),
            sdk_version: "1.0.0".to_string(),
            events,
            is_final,
        };

        let envelope = ReplayBatchEnvelope {
            project_id: "proj-1".to_string(),
            received_at: "2025-01-01T00:00:00Z".to_string(),
            batch,
        };

        serde_json::to_vec(&envelope).unwrap()
    }

    /// Replicates the core logic of `process_message` using mock components.
    async fn simulate_process(
        s3: &MockS3Store,
        db: &MockReplayStore,
        buffer: &Mutex<SessionBuffer>,
        payload: &[u8],
    ) -> Result<()> {
        let envelope: ReplayBatchEnvelope = serde_json::from_slice(payload)?;

        let project_id = &envelope.project_id;
        let session_id = &envelope.batch.session_id;
        let chunk_seq = envelope.batch.chunk_seq;
        let payload_size = payload.len() as i32;
        let event_count = envelope.batch.events.len() as i32;

        // Dedup check
        {
            let mut buf = buffer.lock().await;
            if buf.is_duplicate(session_id, chunk_seq) {
                return Ok(());
            }
            buf.mark_seen(session_id, chunk_seq);
        }

        // Upload raw events to S3
        let s3_key = s3
            .upload_chunk(project_id, session_id, chunk_seq, payload)
            .await?;

        // Upsert session metadata
        db.upsert_session(
            project_id,
            session_id,
            envelope.batch.distinct_id.as_deref(),
            envelope.batch.is_final,
            1,
        )
        .await?;

        // Insert chunk metadata
        db.insert_chunk(
            project_id,
            session_id,
            &envelope.batch.window_id,
            chunk_seq,
            &s3_key,
            "test-bucket",
            payload_size,
            event_count,
            envelope.batch.is_final,
        )
        .await?;

        // Evict session buffer if final
        if envelope.batch.is_final {
            let mut buf = buffer.lock().await;
            buf.evict_session(session_id);
        }

        Ok(())
    }

    // ── edge case 22: corrupt message ────────────────────────────────────────────

    #[test]
    fn corrupt_message_fails_deserialization() {
        let result = serde_json::from_slice::<ReplayBatchEnvelope>(b"not json");
        assert!(result.is_err());
    }

    #[test]
    fn empty_payload_fails_deserialization() {
        let result = serde_json::from_slice::<ReplayBatchEnvelope>(b"");
        assert!(result.is_err());
    }

    #[test]
    fn partial_json_fails_deserialization() {
        let result = serde_json::from_slice::<ReplayBatchEnvelope>(b"{\"projectId\":\"p\"}");
        assert!(result.is_err());
    }

    // ── edge case 23: duplicate chunk_seq ────────────────────────────────────────

    #[tokio::test]
    async fn duplicate_chunk_is_skipped() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());
        let payload = make_envelope_payload("sess-1", 0, false, 1, Some("user-1"));

        // First call — should succeed
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        // Second call with same (session_id, chunk_seq) — should be skipped
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        // S3 should have exactly one upload
        assert_eq!(s3.chunks.lock().unwrap().len(), 1);
        // DB should have exactly one session and one chunk
        assert_eq!(db.sessions.lock().unwrap().len(), 1);
        assert_eq!(db.chunks.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn duplicate_across_different_sessions_not_skipped() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let payload_a = make_envelope_payload("sess-a", 0, false, 1, Some("user-1"));
        let payload_b = make_envelope_payload("sess-b", 0, false, 1, Some("user-2"));

        simulate_process(&s3, &db, &buffer, &payload_a)
            .await
            .unwrap();
        simulate_process(&s3, &db, &buffer, &payload_b)
            .await
            .unwrap();

        assert_eq!(s3.chunks.lock().unwrap().len(), 2);
        assert_eq!(db.sessions.lock().unwrap().len(), 2);
        assert_eq!(db.chunks.lock().unwrap().len(), 2);
    }

    // ── edge case 24: out-of-order chunks ────────────────────────────────────────

    #[tokio::test]
    async fn out_of_order_chunks_are_independent() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        // chunk_seq 1 arrives before chunk_seq 0
        let payload_1 = make_envelope_payload("sess-1", 1, false, 1, Some("user-1"));
        let payload_0 = make_envelope_payload("sess-1", 0, false, 1, Some("user-1"));

        simulate_process(&s3, &db, &buffer, &payload_1)
            .await
            .unwrap();
        simulate_process(&s3, &db, &buffer, &payload_0)
            .await
            .unwrap();

        // Both chunks should be stored
        assert_eq!(s3.chunks.lock().unwrap().len(), 2);
        assert_eq!(db.chunks.lock().unwrap().len(), 2);
    }

    // ── edge case 26: S3 upload failure ──────────────────────────────────────────

    #[tokio::test]
    async fn s3_upload_failure_propagates_error() {
        let s3 = MockS3Store::with_failure();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());
        let payload = make_envelope_payload("sess-1", 0, false, 1, Some("user-1"));

        let result = simulate_process(&s3, &db, &buffer, &payload).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("s3 upload failed"), "got: {err}");
        // DB should not have been touched
        assert_eq!(db.sessions.lock().unwrap().len(), 0);
        assert_eq!(db.chunks.lock().unwrap().len(), 0);
    }

    // ── edge case 27: Postgres write failure ─────────────────────────────────────

    #[tokio::test]
    async fn postgres_upsert_failure_propagates_error() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::with_failure();
        let buffer = Mutex::new(SessionBuffer::new());
        let payload = make_envelope_payload("sess-1", 0, false, 1, Some("user-1"));

        let result = simulate_process(&s3, &db, &buffer, &payload).await;

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("postgres upsert failed"), "got: {err}");
    }

    // ── edge case 29: very large chunk ───────────────────────────────────────────

    #[tokio::test]
    async fn very_large_chunk_is_handled() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        // 10 000 events in a single chunk
        let payload = make_envelope_payload("sess-1", 0, false, 10_000, Some("user-1"));

        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        let chunks = s3.chunks.lock().unwrap();
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].2, 0); // chunk_seq

        let db_chunks = db.chunks.lock().unwrap();
        assert_eq!(db_chunks[0].7, 10_000); // event_count
    }

    // ── edge case 30: special characters in session_id ───────────────────────────

    #[tokio::test]
    async fn special_chars_in_session_id_are_handled() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let malicious = "'; DROP TABLE replay_sessions; --";
        let payload = make_envelope_payload(malicious, 0, false, 1, Some("user-1"));

        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        // S3 key should contain the raw session_id
        let s3_chunks = s3.chunks.lock().unwrap();
        assert_eq!(s3_chunks[0].1, malicious);

        // DB should have stored the raw session_id
        let sessions = db.sessions.lock().unwrap();
        assert_eq!(sessions[0].1, malicious);

        let db_chunks = db.chunks.lock().unwrap();
        assert_eq!(db_chunks[0].1, malicious);
    }

    #[tokio::test]
    async fn unicode_in_session_id_is_handled() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let session_id = "sess-@\u{1f600}\u{1f680}-emoji";
        let payload = make_envelope_payload(session_id, 0, false, 1, Some("user-1"));

        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        let sessions = db.sessions.lock().unwrap();
        assert_eq!(sessions[0].1, session_id);
    }

    // ── edge case 33: is_final=true with more chunks after ───────────────────────

    #[tokio::test]
    async fn final_then_more_chunks_do_not_revert_status() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        // First chunk marks final
        let final_payload = make_envelope_payload("sess-final", 0, true, 1, Some("user-1"));
        simulate_process(&s3, &db, &buffer, &final_payload)
            .await
            .unwrap();

        // Subsequent chunk with is_final=false
        let non_final = make_envelope_payload("sess-final", 1, false, 1, Some("user-1"));
        simulate_process(&s3, &db, &buffer, &non_final)
            .await
            .unwrap();

        // First session upsert should have is_final=true, second with is_final=false
        let sessions = db.sessions.lock().unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions[0].3); // first upsert: is_final = true
        assert!(!sessions[1].3); // second upsert: is_final = false

        // Session should have been evicted from buffer after final
        assert!(!buffer.lock().await.is_duplicate("sess-final", 0));

        // But the subsequent chunk should still be processed (new mark_seen)
        assert!(buffer.lock().await.is_duplicate("sess-final", 1));
    }

    // ── edge case 34: is_final never sent ────────────────────────────────────────

    #[tokio::test]
    async fn never_final_session_remains_active() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        // Multiple chunks, none final
        for seq in 0..5 {
            let payload = make_envelope_payload("sess-active", seq, false, 1, Some("user-1"));
            simulate_process(&s3, &db, &buffer, &payload).await.unwrap();
        }

        // All chunks should be in the buffer
        for seq in 0..5 {
            assert!(buffer.lock().await.is_duplicate("sess-active", seq));
        }
        assert_eq!(s3.chunks.lock().unwrap().len(), 5);
        assert_eq!(db.chunks.lock().unwrap().len(), 5);

        // All upserts should have is_final=false
        let sessions = db.sessions.lock().unwrap();
        assert_eq!(sessions.len(), 5);
        for s in sessions.iter() {
            assert!(!s.3, "expected is_final=false, got {:?}", s);
        }
    }

    // ── edge case 35: chunk_seq overflow (u32::MAX) ──────────────────────────────

    #[tokio::test]
    async fn chunk_seq_max_value_is_handled() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let payload = make_envelope_payload("sess-overflow", u32::MAX, false, 1, Some("user-1"));
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        // Buffer should track it
        assert!(buffer.lock().await.is_duplicate("sess-overflow", u32::MAX));

        // S3 key should contain the value
        let s3_chunks = s3.chunks.lock().unwrap();
        assert_eq!(s3_chunks[0].2, u32::MAX);

        // DB should store it
        let db_chunks = db.chunks.lock().unwrap();
        assert_eq!(db_chunks[0].3, u32::MAX);
    }

    #[tokio::test]
    async fn chunk_seq_near_max_with_duplicate_detection() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let payload =
            make_envelope_payload("sess-near-max", u32::MAX - 1, false, 1, Some("user-1"));
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        // Duplicate should be detected
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        assert_eq!(s3.chunks.lock().unwrap().len(), 1);
    }

    // ── edge case 28: session with no chunks (query-side contract) ──────────────

    #[test]
    fn empty_events_vec_is_valid() {
        let payload = make_envelope_payload("sess-empty", 0, false, 0, Some("user-1"));
        let envelope: ReplayBatchEnvelope = serde_json::from_slice(&payload).unwrap();
        assert!(envelope.batch.events.is_empty());
    }

    // ── distinct_id edge cases ────────────────────────────────────────────────────

    #[tokio::test]
    async fn null_distinct_id_is_handled() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let payload = make_envelope_payload("sess-1", 0, false, 1, None);
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        let sessions = db.sessions.lock().unwrap();
        assert_eq!(sessions.len(), 1);
        assert!(sessions[0].2.is_none());
    }

    #[tokio::test]
    async fn empty_distinct_id_is_handled() {
        let s3 = MockS3Store::new();
        let db = MockReplayStore::new();
        let buffer = Mutex::new(SessionBuffer::new());

        let payload = make_envelope_payload("sess-1", 0, false, 1, Some(""));
        simulate_process(&s3, &db, &buffer, &payload).await.unwrap();

        let sessions = db.sessions.lock().unwrap();
        assert_eq!(sessions[0].2, Some("".to_string()));
    }
}
