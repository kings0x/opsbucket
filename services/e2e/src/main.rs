use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::sleep;
use tracing::info;

mod helpers;
mod scenarios;
mod setup;

// ── Constants ──────────────────────────────────────────────────────

pub(crate) fn root_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root should exist")
}

pub(crate) fn services_dir() -> PathBuf {
    root_dir().join("services")
}

pub(crate) const INGEST_PORT: u16 = 8080;
pub(crate) const QUERY_PORT: u16 = 8081;
pub(crate) const HOST_LOOPBACK: &str = "127.0.0.1";
pub(crate) const KAFKA_BROKERS: &str = "127.0.0.1:9092";
pub(crate) const DATABASE_URL: &str = "postgres://opsbucket:opsbucket@127.0.0.1:5432/opsbucket";
pub(crate) const REDIS_URL: &str = "redis://127.0.0.1:6379";
pub(crate) const CLICKHOUSE_URL: &str = "http://127.0.0.1:8123";

pub(crate) const WRITE_KEY: &str = "wk_test_valid_key_12345";
pub(crate) const PROJECT_ID: &str = "proj_test";
pub(crate) const SECRET_KEY: &str = "test-query-secret-key";

pub(crate) const TIMEOUT_SECS: u64 = 120;

pub(crate) const PG_CONTAINER: &str = "postgres";
pub(crate) const CH_CONTAINER: &str = "clickhouse";
pub(crate) const RP_CONTAINER: &str = "redpanda";
pub(crate) const REDIS_CONTAINER: &str = "redis";
pub(crate) const MINIO_CONTAINER: &str = "minio";
pub(crate) const MINIO_ENDPOINT: &str = "http://127.0.0.1:9002";
pub(crate) const ARCHIVE_S3_BUCKET: &str = "opsbucket-archive";

// ── Stats ──────────────────────────────────────────────────────────

pub(crate) static PASS: AtomicUsize = AtomicUsize::new(0);
pub(crate) static FAIL: AtomicUsize = AtomicUsize::new(0);

#[macro_export]
macro_rules! pass {
    ($($arg:tt)*) => {{
        $crate::PASS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        println!("  \u{2713} PASS: {}", format!($($arg)*));
    }};
}

#[macro_export]
macro_rules! fail {
    ($($arg:tt)*) => {{
        $crate::FAIL.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        println!("  \u{2717} FAIL: {}", format!($($arg)*));
    }};
}

// ── Guards ─────────────────────────────────────────────────────────

pub(crate) struct ServiceGuard(Option<Child>);

impl ServiceGuard {
    pub(crate) fn new(child: Child) -> Self {
        Self(Some(child))
    }
}

impl Drop for ServiceGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub(crate) struct ComposeGuard {
    no_cleanup: bool,
}

impl ComposeGuard {
    pub(crate) fn new(no_cleanup: bool) -> Self {
        Self { no_cleanup }
    }
}

impl Drop for ComposeGuard {
    fn drop(&mut self) {
        if self.no_cleanup {
            return;
        }
        let compose_file = root_dir().join("infra").join("docker-compose.yml");
        let compose_file_arg = compose_file.to_string_lossy().into_owned();
        info!("stopping infrastructure...");
        let _ = Command::new("docker")
            .args([
                "compose",
                "-p",
                "opsbucket-e2e",
                "-f",
                &compose_file_arg,
                "down",
                "-v",
            ])
            .status();
    }
}

// ── Main ──────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("opsbucket_e2e=info".parse().unwrap())
                .add_directive("info".parse().unwrap()),
        )
        .init();

    let args: Vec<String> = std::env::args().collect();
    let skip_build = args.contains(&"--skip-build".to_string());
    let no_cleanup = args.contains(&"--no-cleanup".to_string());

    // ── Phase 1: Infrastructure ──
    println!("\n\x1b[1m── Phase 1: Infrastructure (docker compose) ──\x1b[0m\n");
    setup::start_infrastructure()?;
    let _compose_guard = ComposeGuard::new(no_cleanup);
    setup::wait_for_infrastructure().await?;

    // ── Phase 2: Configuration ──
    println!("\n\x1b[1m── Phase 2: Configuration ──\x1b[0m\n");
    setup::create_kafka_topics()?;
    setup::run_migrations()?;
    setup::seed_test_data()?;
    setup::flush_redis()?;
    setup::apply_clickhouse_schema()?;
    setup::create_archive_bucket()?;

    // ── Phase 3: Build ──
    println!("\n\x1b[1m── Phase 3: Build ──\x1b[0m\n");
    if !skip_build {
        setup::build_services()?;
    } else {
        info!("skipping build (--skip-build)");
    }

    // ── Phase 4: Start Services ──
    println!("\n\x1b[1m── Phase 4: Start Services ──\x1b[0m\n");
    let (_ingest_guard, _processing_guard, _query_guard, _archiver_guard) =
        setup::start_services()?;

    helpers::wait_for_port(HOST_LOOPBACK, INGEST_PORT, "Ingestion", 30).await?;
    info!("processing waiting 5s for initial poll...");
    sleep(Duration::from_secs(5)).await;
    helpers::wait_for_port(HOST_LOOPBACK, QUERY_PORT, "Query Service", 30).await?;

    // ── Phase 5: Test Scenarios ──
    println!("\n\x1b[1m── Phase 5: Test Scenarios ──\x1b[0m\n");

    // let Kafka producers settle metadata discovery
    sleep(Duration::from_secs(3)).await;

    scenarios::scenario_track_event().await?;
    scenarios::scenario_identify_and_resolution().await?;
    scenarios::scenario_page_event().await?;
    scenarios::scenario_dedup().await?;
    scenarios::scenario_invalid_write_key().await?;

    info!("waiting 10s for processing to consume events...");
    sleep(Duration::from_secs(10)).await;

    scenarios::scenario_verify_clickhouse().await?;
    scenarios::scenario_verify_postgres().await?;

    scenarios::scenario_query_health().await?;
    scenarios::scenario_query_auth().await?;
    scenarios::scenario_query_funnel().await?;
    scenarios::scenario_query_retention().await?;
    scenarios::scenario_query_segment().await?;
    scenarios::scenario_query_raw_events().await?;
    scenarios::scenario_query_validation().await?;
    scenarios::scenario_identify_multiple().await?;
    scenarios::scenario_missing_identity_fields().await?;
    scenarios::scenario_negative_timestamp_skew().await?;
    scenarios::scenario_segment_operators().await?;
    scenarios::scenario_dlq_path().await?;
    scenarios::scenario_archive().await?;
    scenarios::scenario_concurrent_ingestion().await?;

    // ── Summary ──
    let pass = PASS.load(Ordering::SeqCst);
    let fail = FAIL.load(Ordering::SeqCst);
    let total = pass + fail;
    println!("\n\x1b[1m═══════════════════════════════════════════\x1b[0m");
    println!(
        "\x1b[1m  Results: {} passed, {} failed out of {} total\x1b[0m",
        pass, fail, total
    );
    println!("\x1b[1m═══════════════════════════════════════════\x1b[0m");

    if fail > 0 {
        anyhow::bail!("{} tests failed", fail);
    }

    Ok(())
}
