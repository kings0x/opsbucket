use std::sync::Arc;
use std::sync::OnceLock;

use axum::Router;
use chrono::{DateTime, Duration, Utc};
use clickhouse::Row;
use opsbucket_query::config::Config;
use opsbucket_query::AppState;
use serde::Serialize;
use sqlx::PgPool;

static SETUP: OnceLock<()> = OnceLock::new();

pub fn setup_env() {
    SETUP.get_or_init(|| {
        if std::env::var("SECRET_KEY").is_err() {
            std::env::set_var("SECRET_KEY", "test-secret-key");
        }
        if std::env::var("DATABASE_URL").is_err() {
            std::env::set_var(
                "DATABASE_URL",
                "postgres://opsbucket:opsbucket@localhost:5432/opsbucket",
            );
        }
        if std::env::var("REDIS_URL").is_err() {
            std::env::set_var("REDIS_URL", "redis://localhost:6379");
        }
        if std::env::var("CLICKHOUSE_URL").is_err() {
            std::env::set_var("CLICKHOUSE_URL", "http://localhost:8123/default");
        }
        let _ = tracing_subscriber::fmt().with_env_filter("off").try_init();
    });
}

pub fn clickhouse_client() -> clickhouse::Client {
    setup_env();
    clickhouse::Client::default()
        .with_url("http://localhost:8123")
        .with_user("default")
        .with_password("opsbucket")
        .with_database("default")
}

pub async fn pg_pool() -> PgPool {
    setup_env();
    PgPool::connect("postgres://opsbucket:opsbucket@localhost:5432/opsbucket")
        .await
        .expect("connect to postgres")
}

pub async fn redis_manager() -> redis::aio::ConnectionManager {
    setup_env();
    let client = redis::Client::open("redis://localhost:6379").expect("open redis");
    client
        .get_connection_manager()
        .await
        .expect("redis connection manager")
}

pub fn build_router(state: Arc<AppState>) -> Router {
    use axum::routing::{get, post};
    use tower_http::cors::CorsLayer;
    use tower_http::trace::TraceLayer;

    Router::new()
        .route(
            "/v1/query/funnel",
            post(opsbucket_query::routes::funnel::handler),
        )
        .route(
            "/v1/query/retention",
            post(opsbucket_query::routes::retention::handler),
        )
        .route(
            "/v1/query/segment",
            post(opsbucket_query::routes::segment::handler),
        )
        .route(
            "/v1/query/events",
            get(opsbucket_query::routes::events::handler),
        )
        .route("/health", get(opsbucket_query::routes::health::handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn build_test_state(config_override: Option<Config>) -> Arc<AppState> {
    setup_env();
    let config = config_override.unwrap_or_else(|| Config {
        secret_key: "test-secret-key".into(),
        database_url: "postgres://opsbucket:opsbucket@localhost:5432/opsbucket".into(),
        redis_url: "redis://localhost:6379".into(),
        clickhouse_url: "http://localhost:8123/default".into(),
        clickhouse_user: "default".into(),
        clickhouse_password: "".into(),
        query_cache_ttl_seconds: 60,
        query_timeout_seconds: 30,
        max_date_range_days: 366,
        port: 0,
        rust_log: "off".into(),
    });

    let ch = clickhouse::Client::default()
        .with_url("http://localhost:8123")
        .with_user("default")
        .with_password("opsbucket")
        .with_database("default");

    let pg = PgPool::connect(&config.database_url)
        .await
        .expect("connect postgres");

    let redis_client = redis::Client::open(config.redis_url.as_str()).expect("open redis");
    let redis = redis_client
        .get_connection_manager()
        .await
        .expect("redis connection");

    Arc::new(AppState {
        secret_key: config.secret_key.clone(),
        pg,
        redis,
        ch_client: ch,
        config,
    })
}

// ── Fixture data ────────────────────────────────────────────────────
// Base timestamp: Monday 2026-06-15 00:00:00 UTC

pub fn base_ts() -> DateTime<Utc> {
    "2026-06-15T00:00:00Z".parse::<DateTime<Utc>>().unwrap()
}

pub const PROJ_1: &str = "proj_test_1";
pub const PROJ_2: &str = "proj_test_2";

#[derive(Row, Serialize)]
pub struct ChEventRow {
    pub project_id: String,
    pub event_id: String,
    pub event_name: String,
    pub event_type: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub original_timestamp: DateTime<Utc>,
    pub properties: Vec<(String, String)>,
    pub traits: Vec<(String, String)>,
    pub user_agent: String,
    pub locale: String,
    pub timezone: String,
    pub ip: String,
    pub library_name: String,
    pub library_version: String,
    pub page_url: String,
    pub page_path: String,
    pub page_referrer: String,
    pub page_title: String,
    pub page_search: String,
    pub screen_width: u16,
    pub screen_height: u16,
    pub screen_density: f32,
    pub campaign_source: Option<String>,
    pub campaign_medium: Option<String>,
    pub campaign_name: Option<String>,
    pub campaign_term: Option<String>,
    pub campaign_content: Option<String>,
}

impl ChEventRow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        project_id: &str,
        event_id: &str,
        event_name: &str,
        event_type: &str,
        anonymous_id: &str,
        user_id: Option<&str>,
        timestamp: DateTime<Utc>,
        properties: Vec<(String, String)>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            event_id: event_id.into(),
            event_name: event_name.into(),
            event_type: event_type.into(),
            anonymous_id: anonymous_id.into(),
            user_id: user_id.map(String::from),
            timestamp,
            received_at: timestamp,
            original_timestamp: timestamp,
            properties,
            traits: Vec::new(),
            user_agent: "test-agent".into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            ip: "127.0.0.1".into(),
            library_name: "opsbucket-sdk".into(),
            library_version: "1.0.0".into(),
            page_url: "https://example.com/page".into(),
            page_path: "/page".into(),
            page_referrer: String::new(),
            page_title: "Test Page".into(),
            page_search: String::new(),
            screen_width: 1920,
            screen_height: 1080,
            screen_density: 2.0,
            campaign_source: None,
            campaign_medium: None,
            campaign_name: None,
            campaign_term: None,
            campaign_content: None,
        }
    }
}

pub async fn insert_funnel_fixtures(ch: &clickhouse::Client) {
    let base = base_ts();
    let rows = vec![
        // anon_a: completes all 3 funnel steps within 3600s window
        ChEventRow::new(
            PROJ_1,
            "ev_a1",
            "Page Viewed",
            "page",
            "anon_a",
            None,
            base,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_a2",
            "Button Clicked",
            "track",
            "anon_a",
            None,
            base + Duration::minutes(30),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_a3",
            "Account Created",
            "track",
            "anon_a",
            None,
            base + Duration::minutes(60),
            Vec::new(),
        ),
        // anon_b: completes 2 funnel steps (no Account Created)
        ChEventRow::new(
            PROJ_1,
            "ev_b1",
            "Page Viewed",
            "page",
            "anon_b",
            None,
            base,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_b2",
            "Button Clicked",
            "track",
            "anon_b",
            None,
            base + Duration::minutes(120),
            Vec::new(),
        ),
        // anon_c: completes only 1 funnel step
        ChEventRow::new(
            PROJ_1,
            "ev_c1",
            "Page Viewed",
            "page",
            "anon_c",
            None,
            base,
            Vec::new(),
        ),
        // anon_d: completes all 3 but step 3 outside window (at +120min, window=3600)
        ChEventRow::new(
            PROJ_1,
            "ev_d1",
            "Page Viewed",
            "page",
            "anon_d",
            None,
            base,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_d2",
            "Button Clicked",
            "track",
            "anon_d",
            None,
            base + Duration::minutes(30),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_d3",
            "Account Created",
            "track",
            "anon_d",
            None,
            base + Duration::minutes(120),
            Vec::new(),
        ),
        // anon_e: has a user_id set (pre-identified)
        ChEventRow::new(
            PROJ_1,
            "ev_e1",
            "Page Viewed",
            "page",
            "anon_e",
            Some("usr_e"),
            base,
            Vec::new(),
        ),
        // anon_f: many "Feature Used" events for segment testing
        ChEventRow::new(
            PROJ_1,
            "ev_f1",
            "Feature Used",
            "track",
            "anon_f",
            None,
            base,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_f2",
            "Feature Used",
            "track",
            "anon_f",
            None,
            base + Duration::hours(1),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_f3",
            "Feature Used",
            "track",
            "anon_f",
            None,
            base + Duration::hours(2),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_f4",
            "Feature Used",
            "track",
            "anon_f",
            None,
            base + Duration::hours(3),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_f5",
            "Feature Used",
            "track",
            "anon_f",
            None,
            base + Duration::hours(4),
            Vec::new(),
        ),
        // anon_g: Identify event with traits, then Feature Used
        {
            let mut props = Vec::new();
            props.push(("plan".into(), "pro".into()));
            ChEventRow::new(
                PROJ_1,
                "ev_g1",
                "Identify",
                "identify",
                "anon_g",
                Some("usr_g"),
                base,
                props,
            )
        },
        // anon_h: Has "free" plan, 2 Feature Used events
        {
            let mut props = Vec::new();
            props.push(("plan".into(), "free".into()));
            ChEventRow::new(
                PROJ_1,
                "ev_h1",
                "Identify",
                "identify",
                "anon_h",
                None,
                base + Duration::days(1),
                props,
            )
        },
        ChEventRow::new(
            PROJ_1,
            "ev_h2",
            "Feature Used",
            "track",
            "anon_h",
            None,
            base + Duration::days(1) + Duration::hours(1),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_h3",
            "Feature Used",
            "track",
            "anon_h",
            None,
            base + Duration::days(1) + Duration::hours(2),
            Vec::new(),
        ),
    ];

    let mut insert = ch.insert("events").expect("create inserter");
    for row in rows {
        insert.write(&row).await.expect("write fixture row");
    }
    insert.end().await.expect("flush fixtures");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

pub async fn insert_retention_fixtures(ch: &clickhouse::Client) {
    let week1 = "2026-06-01T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    let week2 = "2026-06-08T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    let week3 = "2026-06-15T00:00:00Z".parse::<DateTime<Utc>>().unwrap();
    let week4 = "2026-06-22T00:00:00Z".parse::<DateTime<Utc>>().unwrap();

    let rows = vec![
        // anon_r1: first seen week1, returns week2, week4
        ChEventRow::new(
            PROJ_2,
            "ev_r1a",
            "App Opened",
            "track",
            "anon_r1",
            None,
            week1,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_2,
            "ev_r1b",
            "App Opened",
            "track",
            "anon_r1",
            None,
            week2,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_2,
            "ev_r1c",
            "App Opened",
            "track",
            "anon_r1",
            None,
            week4,
            Vec::new(),
        ),
        // anon_r2: first seen week1, returns week3
        ChEventRow::new(
            PROJ_2,
            "ev_r2a",
            "App Opened",
            "track",
            "anon_r2",
            None,
            week1,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_2,
            "ev_r2b",
            "App Opened",
            "track",
            "anon_r2",
            None,
            week3,
            Vec::new(),
        ),
        // anon_r3: first seen week1, never returns
        ChEventRow::new(
            PROJ_2,
            "ev_r3a",
            "App Opened",
            "track",
            "anon_r3",
            None,
            week1,
            Vec::new(),
        ),
        // anon_r4: first seen week2, returns week3
        ChEventRow::new(
            PROJ_2,
            "ev_r4a",
            "App Opened",
            "track",
            "anon_r4",
            None,
            week2,
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_2,
            "ev_r4b",
            "App Opened",
            "track",
            "anon_r4",
            None,
            week3,
            Vec::new(),
        ),
        // anon_r5: first seen week4, no returns
        ChEventRow::new(
            PROJ_2,
            "ev_r5a",
            "App Opened",
            "track",
            "anon_r5",
            None,
            week4,
            Vec::new(),
        ),
    ];

    let mut insert = ch.insert("events").expect("create inserter");
    for row in rows {
        insert.write(&row).await.expect("write retention fixture");
    }
    insert.end().await.expect("flush retention fixtures");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

pub async fn insert_page_event_fixtures(ch: &clickhouse::Client) {
    let base = base_ts();
    let rows = vec![
        ChEventRow::new(
            PROJ_1,
            "ev_p1",
            "Page Viewed",
            "page",
            "anon_p",
            None,
            base + Duration::hours(10),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p2",
            "Page Viewed",
            "page",
            "anon_p",
            None,
            base + Duration::hours(9),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p3",
            "Page Viewed",
            "page",
            "anon_p",
            None,
            base + Duration::hours(8),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p4",
            "Page Viewed",
            "page",
            "anon_p",
            None,
            base + Duration::hours(7),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p5",
            "Page Viewed",
            "page",
            "anon_p",
            None,
            base + Duration::hours(6),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p6",
            "Signup Flow",
            "track",
            "anon_p",
            None,
            base + Duration::hours(5),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p7",
            "Signup Flow",
            "track",
            "anon_p",
            None,
            base + Duration::hours(4),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p8",
            "Dashboard View",
            "track",
            "anon_user1",
            Some("usr_query"),
            base + Duration::hours(3),
            Vec::new(),
        ),
        ChEventRow::new(
            PROJ_1,
            "ev_p9",
            "Dashboard View",
            "track",
            "anon_user1",
            Some("usr_query"),
            base + Duration::hours(2),
            Vec::new(),
        ),
    ];

    let mut insert = ch.insert("events").expect("create inserter");
    for row in rows {
        insert.write(&row).await.expect("write page event fixture");
    }
    insert.end().await.expect("flush page event fixtures");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
}

pub async fn insert_identity_fixtures(pg: &PgPool) {
    sqlx::query(
        "INSERT INTO identity_aliases (project_id, anonymous_id, user_id, created_at)
         VALUES ($1, $2, $3, NOW())
         ON CONFLICT (project_id, anonymous_id) DO UPDATE SET user_id = $3",
    )
    .bind("proj_test_1")
    .bind("anon_missing")
    .bind("usr_resolved_late")
    .execute(pg)
    .await
    .expect("insert identity alias");
}

pub async fn clean_clickhouse(ch: &clickhouse::Client) {
    ch.query("TRUNCATE TABLE IF EXISTS events")
        .execute()
        .await
        .expect("truncate events");
}

pub async fn clean_identity(pg: &PgPool) {
    sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_test_1'")
        .execute(pg)
        .await
        .expect("delete identity aliases");
}

pub async fn clean_redis(redis: &mut redis::aio::ConnectionManager) {
    let _: Result<(), _> = redis::Cmd::new().arg("FLUSHALL").query_async(redis).await;
}
