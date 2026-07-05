use std::sync::OnceLock;

use clickhouse::Client as ChClient;
use opsbucket_processing::kafka::consumer::ProcessingConsumer;
use opsbucket_shared::events::RawEvent;
use rdkafka::producer::FutureProducer;
use rdkafka::ClientConfig;
use redis::aio::ConnectionManager as RedisConnectionManager;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

const KAFKA_BROKERS: &str = "127.0.0.1:9092";
const PG_URL: &str = "postgres://opsbucket:opsbucket@127.0.0.1:5432/opsbucket";
const REDIS_URL: &str = "redis://127.0.0.1:6379";
const CLICKHOUSE_URL: &str = "http://127.0.0.1:8123";
const CLICKHOUSE_DB: &str = "default";

static SETUP: OnceLock<()> = OnceLock::new();

pub fn setup_env() {
    SETUP.get_or_init(|| {
        if std::env::var("KAFKA_BROKERS").is_err() {
            std::env::set_var("KAFKA_BROKERS", KAFKA_BROKERS);
        }
        if std::env::var("DATABASE_URL").is_err() {
            std::env::set_var("DATABASE_URL", PG_URL);
        }
        if std::env::var("REDIS_URL").is_err() {
            std::env::set_var("REDIS_URL", REDIS_URL);
        }
        if std::env::var("CLICKHOUSE_URL").is_err() {
            std::env::set_var(
                "CLICKHOUSE_URL",
                format!("{CLICKHOUSE_URL}/{CLICKHOUSE_DB}"),
            );
        }
        let _ = tracing_subscriber::fmt().with_env_filter("off").try_init();
    });
}

pub fn clickhouse_client() -> ChClient {
    setup_env();
    ChClient::default()
        .with_url("http://127.0.0.1:8123")
        .with_user("default")
        .with_password("opsbucket")
        .with_database("default")
}

pub async fn pg_pool() -> PgPool {
    setup_env();
    PgPoolOptions::new()
        .max_connections(5)
        .connect(PG_URL)
        .await
        .expect("connect to postgres")
}

pub async fn redis_connection() -> RedisConnectionManager {
    setup_env();
    let client = redis::Client::open(REDIS_URL).expect("open redis");
    RedisConnectionManager::new(client)
        .await
        .expect("redis connection")
}

pub fn make_track_event(message_id: &str) -> RawEvent {
    RawEvent {
        project_id: "proj_test_integration".into(),
        received_at: "2026-07-05T12:00:00Z".into(),
        sent_at: "2026-07-05T12:00:00Z".into(),
        ip: "127.0.0.1".into(),
        message_id: message_id.into(),
        event_type: "track".into(),
        anonymous_id: "anon_integration".into(),
        user_id: None,
        original_timestamp: "2026-07-05T11:59:55Z".into(),
        context: opsbucket_shared::events::Context {
            library: opsbucket_shared::events::Library {
                name: "@opsbucket/browser".into(),
                version: "0.1.0".into(),
            },
            page: opsbucket_shared::events::Page {
                url: "https://example.com".into(),
                path: "/".into(),
                referrer: "".into(),
                title: "Home".into(),
                search: "".into(),
            },
            screen: opsbucket_shared::events::Screen {
                width: 1440,
                height: 900,
                density: 2.0,
            },
            user_agent: "test-agent".into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            campaign: opsbucket_shared::events::Campaign {
                source: None,
                medium: None,
                name: None,
                term: None,
                content: None,
            },
            ip: None,
        },
        event: Some("Button Clicked".into()),
        properties: Some(serde_json::json!({"button_text": "Sign Up"})),
        traits: None,
        name: None,
    }
}

pub fn make_identify_event(message_id: &str, anonymous_id: &str, user_id: &str) -> RawEvent {
    RawEvent {
        project_id: "proj_test_integration".into(),
        received_at: "2026-07-05T12:00:00Z".into(),
        sent_at: "2026-07-05T12:00:00Z".into(),
        ip: "127.0.0.1".into(),
        message_id: message_id.into(),
        event_type: "identify".into(),
        anonymous_id: anonymous_id.into(),
        user_id: Some(user_id.into()),
        original_timestamp: "2026-07-05T11:59:55Z".into(),
        context: opsbucket_shared::events::Context {
            library: opsbucket_shared::events::Library {
                name: "@opsbucket/browser".into(),
                version: "0.1.0".into(),
            },
            page: opsbucket_shared::events::Page {
                url: "https://example.com".into(),
                path: "/".into(),
                referrer: "".into(),
                title: "Home".into(),
                search: "".into(),
            },
            screen: opsbucket_shared::events::Screen {
                width: 1440,
                height: 900,
                density: 2.0,
            },
            user_agent: "test-agent".into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            campaign: opsbucket_shared::events::Campaign {
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
        traits: Some(serde_json::json!({"plan": "pro", "age": 30})),
        name: None,
    }
}

pub async fn build_consumer() -> ProcessingConsumer {
    setup_env();
    ProcessingConsumer::new(
        KAFKA_BROKERS,
        "opsbucket-processing-test-unit",
        REDIS_URL,
        PG_URL,
        CLICKHOUSE_URL,
        "default",
        "opsbucket",
        100,
        500,
        3600,
        600,
        3,
    )
    .await
    .expect("create processing consumer")
}

pub async fn build_kafka_consumer(group_suffix: &str) -> ProcessingConsumer {
    setup_env();
    let group_id = format!("opsbucket-processing-test-kafka-{group_suffix}");
    ProcessingConsumer::new(
        KAFKA_BROKERS,
        &group_id,
        REDIS_URL,
        PG_URL,
        CLICKHOUSE_URL,
        "default",
        "opsbucket",
        100,
        500,
        3600,
        600,
        3,
    )
    .await
    .expect("create kafka test consumer")
}

pub async fn clean_clickhouse(ch: &ChClient) {
    ch.query("TRUNCATE TABLE IF EXISTS events")
        .execute()
        .await
        .ok();
}

pub async fn clean_redis(redis: &mut RedisConnectionManager) {
    let _: Result<(), _> = redis::Cmd::new().arg("FLUSHALL").query_async(redis).await;
}

pub async fn clean_identity(pg: &PgPool) {
    sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_test_integration'")
        .execute(pg)
        .await
        .ok();
}

pub fn kafka_producer() -> FutureProducer {
    setup_env();
    ClientConfig::new()
        .set("bootstrap.servers", KAFKA_BROKERS)
        .set("message.timeout.ms", "5000")
        .create()
        .expect("create kafka producer")
}

pub async fn count_clickhouse_rows(ch: &ChClient) -> u64 {
    ch.query("SELECT count() FROM events WHERE project_id = 'proj_test_integration'")
        .fetch_one::<u64>()
        .await
        .unwrap_or(0)
}
