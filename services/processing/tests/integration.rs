//! Integration tests for the processing service pipeline.
//!
//! These tests connect to real Docker-infra services: Redpanda (Kafka),
//! Postgres, Redis, and ClickHouse.
//!
//! Run: cargo test -p opsbucket-processing --test e2e -- --test-threads=1

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use clickhouse::Client as ChClient;
use common::*;
use opsbucket_shared::events::RawEvent;
use rdkafka::consumer::Consumer;
use rdkafka::Message;

// ────────────────────────────────────────────────────────────────────
// process_events direct tests (no Kafka needed for the pipeline itself)
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_process_events_track() {
    let mut consumer = build_consumer().await;
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;
    let pg = pg_pool().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;
    clean_identity(&pg).await;

    let event = make_track_event("track_test_track_1");
    let result = consumer.process_events(vec![event]).await;
    assert!(
        result.is_ok(),
        "process_events should succeed: {:?}",
        result
    );

    let count = count_clickhouse_rows(&ch).await;
    assert_eq!(count, 1, "one row should be in ClickHouse");
}

#[tokio::test]
async fn test_process_events_identify() {
    let mut consumer = build_consumer().await;
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;
    let pg = pg_pool().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;
    clean_identity(&pg).await;

    let event = make_identify_event("identify_test_1", "anon_ident", "usr_ident");
    let result = consumer.process_events(vec![event]).await;
    assert!(
        result.is_ok(),
        "process_events should succeed: {:?}",
        result
    );

    let identity_row: Option<String> = sqlx::query_scalar(
        "SELECT user_id FROM identity_aliases WHERE project_id = $1 AND anonymous_id = $2",
    )
    .bind("proj_test_integration")
    .bind("anon_ident")
    .fetch_optional(&pg)
    .await
    .expect("query identity_aliases");
    assert_eq!(
        identity_row,
        Some("usr_ident".into()),
        "identity alias should exist"
    );

    let count = count_clickhouse_rows(&ch).await;
    assert_eq!(count, 1, "identify event should also be in ClickHouse");
}

#[tokio::test]
async fn test_process_events_dedup_same_batch() {
    let mut consumer = build_consumer().await;
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;
    let pg = pg_pool().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;
    clean_identity(&pg).await;

    let event = make_track_event("dedup_same_batch_1");
    let result = consumer.process_events(vec![event.clone(), event]).await;
    assert!(
        result.is_ok(),
        "process_events should succeed: {:?}",
        result
    );

    let count = count_clickhouse_rows(&ch).await;
    assert_eq!(count, 1, "only one row despite duplicate in batch");
}

#[tokio::test]
async fn test_process_events_dedup_across_batches() {
    let mut consumer = build_consumer().await;
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;
    let pg = pg_pool().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;
    clean_identity(&pg).await;

    let event = make_track_event("dedup_cross_batch_1");
    let r1 = consumer.process_events(vec![event.clone()]).await;
    assert!(r1.is_ok(), "first batch: {:?}", r1);

    let r2 = consumer.process_events(vec![event]).await;
    assert!(r2.is_ok(), "second batch: {:?}", r2);

    let count = count_clickhouse_rows(&ch).await;
    assert_eq!(
        count, 1,
        "cross-batch dedup should prevent duplicate insert"
    );
}

#[tokio::test]
async fn test_process_events_identify_then_resolve() {
    let mut consumer = build_consumer().await;
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;
    let pg = pg_pool().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;
    clean_identity(&pg).await;

    let identify = make_identify_event("resolve_test_id_1", "anon_resolve", "usr_resolve");
    let track = make_track_event("resolve_test_track_1");
    let mut track_with_anon = make_track_event("resolve_test_track_2");
    track_with_anon.anonymous_id = "anon_resolve".into();

    let result = consumer
        .process_events(vec![identify, track_with_anon, track])
        .await;
    assert!(
        result.is_ok(),
        "process_events should succeed: {:?}",
        result
    );

    let user_ids: Vec<String> = sqlx::query_scalar(
        "SELECT user_id FROM identity_aliases WHERE project_id = $1 AND anonymous_id = $2",
    )
    .bind("proj_test_integration")
    .bind("anon_resolve")
    .fetch_all(&pg)
    .await
    .expect("query identity_aliases");
    assert!(!user_ids.is_empty(), "alias should exist");
    assert_eq!(user_ids[0], "usr_resolve");
}

// ────────────────────────────────────────────────────────────────────
// Full consumer integration (requires Kafka)
// ────────────────────────────────────────────────────────────────────

fn make_kafka_event(message_id: &str, project_id: &str) -> RawEvent {
    let mut event = make_track_event(message_id);
    event.project_id = project_id.into();
    event
}

async fn count_kafka_rows(ch: &ChClient, project_id: &str) -> u64 {
    let sql = format!(
        "SELECT count() FROM events WHERE project_id = '{}'",
        project_id
    );
    ch.query(&sql).fetch_one::<u64>().await.unwrap_or(0)
}

#[tokio::test]
async fn test_consumer_processes_from_kafka() {
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;

    let project_id = "proj_kafka_test_1";
    let producer = kafka_producer();
    let event = make_kafka_event("kafka_integration_1", project_id);
    let payload = serde_json::to_vec(&event).expect("serialize event");
    let record = rdkafka::producer::FutureRecord::to("raw-events")
        .payload(&payload)
        .key(&event.message_id);
    producer
        .send(record, std::time::Duration::from_secs(5))
        .await
        .expect("produce to raw-events");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mut consumer = build_kafka_consumer("from_kafka").await;
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let handle = tokio::spawn(async move {
        consumer.run_until(r).await.ok();
    });

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    running.store(false, Ordering::SeqCst);
    handle.await.expect("consumer task joined");

    let count = count_kafka_rows(&ch, project_id).await;
    assert_eq!(count, 1, "consumer should have processed the kafka event");
}

#[tokio::test]
async fn test_consumer_malformed_event_goes_to_dlq() {
    let ch = clickhouse_client();
    let mut redis = redis_connection().await;
    let pg = pg_pool().await;

    clean_clickhouse(&ch).await;
    clean_redis(&mut redis).await;
    clean_identity(&pg).await;

    let project_id = "proj_kafka_dlq_1";

    let producer = kafka_producer();

    let mut good_event = make_track_event("malformed_test_good_1");
    good_event.project_id = project_id.into();
    let good_payload = serde_json::to_vec(&good_event).expect("serialize good event");
    let bad_payload = b"this is not valid json";

    let bad_record = rdkafka::producer::FutureRecord::to("raw-events")
        .payload(bad_payload.as_slice())
        .key("bad_message");
    let good_record = rdkafka::producer::FutureRecord::to("raw-events")
        .payload(&good_payload)
        .key(&good_event.message_id);

    producer
        .send(bad_record, std::time::Duration::from_secs(5))
        .await
        .expect("produce bad message");
    producer
        .send(good_record, std::time::Duration::from_secs(5))
        .await
        .expect("produce good message");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mut consumer = build_kafka_consumer("dlq").await;
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    let handle = tokio::spawn(async move {
        consumer.run_until(r).await.ok();
    });

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    running.store(false, Ordering::SeqCst);
    handle.await.expect("consumer task joined");

    let count = count_kafka_rows(&ch, project_id).await;
    assert_eq!(count, 1, "only the good event should be in ClickHouse");

    let dlq_consumer: rdkafka::consumer::BaseConsumer = rdkafka::ClientConfig::new()
        .set("group.id", "processing-test-dlq-checker")
        .set("bootstrap.servers", "127.0.0.1:9092")
        .set("auto.offset.reset", "earliest")
        .set("enable.auto.commit", "true")
        .create()
        .expect("create dlq consumer");
    dlq_consumer
        .subscribe(&["raw-events-dlq"])
        .expect("subscribe to DLQ");

    let dlq_msg = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match dlq_consumer.poll(std::time::Duration::from_secs(1)) {
                Some(Ok(msg)) => {
                    if let Some(payload) = msg.payload() {
                        let val: serde_json::Value =
                            serde_json::from_slice(payload).unwrap_or_default();
                        if val.get("failed_stage").and_then(|s| s.as_str())
                            == Some("deserialization")
                        {
                            return Some(val);
                        }
                    }
                }
                Some(Err(e)) => {
                    eprintln!("DLQ poll error: {}", e);
                }
                None => {}
            }
        }
    })
    .await;

    let dlq_value = dlq_msg
        .expect("timed out waiting for DLQ message")
        .expect("DLQ message should have a body");
    assert_eq!(dlq_value["failed_stage"], "deserialization");
}
