use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::helpers::{cargo_run, docker_exec, docker_exec_stdin, wait_for_container};
use crate::{
    CH_CONTAINER, PG_CONTAINER, REDIS_CONTAINER, RP_CONTAINER, SERVICES_DIR,
};
use crate::ServiceGuard;

// ── Phase 1: Infrastructure ───────────────────────────────────────

pub(crate) fn start_infrastructure() -> Result<()> {
    let compose_file = format!("{}\\infra\\docker-compose.yml", crate::ROOT_DIR);
    info!("Starting infrastructure via docker compose...");
    let status = Command::new("docker")
        .args([
            "compose",
            "-p",
            "opsbucket-e2e",
            "-f",
            &compose_file,
            "up",
            "-d",
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .status()
        .context("docker compose up failed")?;
    if !status.success() {
        anyhow::bail!("docker compose up failed");
    }
    Ok(())
}

pub(crate) async fn wait_for_infrastructure() -> Result<()> {
    info!("Waiting for Redpanda...");
    wait_for_container(RP_CONTAINER, &["rpk", "cluster", "info"], "Redpanda", 60).await?;

    info!("Waiting for Postgres...");
    wait_for_container(
        PG_CONTAINER,
        &["pg_isready", "-U", "opsbucket"],
        "Postgres",
        30,
    )
    .await?;

    info!("Waiting for Redis...");
    wait_for_container(REDIS_CONTAINER, &["redis-cli", "ping"], "Redis", 15).await?;

    info!("Waiting for ClickHouse...");
    wait_for_container(
        CH_CONTAINER,
        &["clickhouse-client", "--query", "SELECT 1"],
        "ClickHouse",
        30,
    )
    .await?;

    info!("All infrastructure ready");
    Ok(())
}

// ── Phase 2: Configuration ────────────────────────────────────────

pub(crate) fn create_kafka_topics() -> Result<()> {
    info!("Creating Kafka topics...");
    for (topic, partitions) in [("raw-events", "12"), ("raw-events-dlq", "1")] {
        let mut last_err = String::new();
        for attempt in 1..=5 {
            std::thread::sleep(std::time::Duration::from_millis(500 * attempt));
            let result = docker_exec(
                RP_CONTAINER,
                &[
                    "rpk",
                    "topic",
                    "create",
                    topic,
                    "--partitions",
                    partitions,
                    "-r",
                    "1",
                ],
            );
            match result {
                Ok(_) => {
                    info!("topic {} created (attempt {})", topic, attempt);
                    break;
                }
                Err(e) => {
                    let msg = e.to_string();
                    if msg.contains("TOPIC_ALREADY_EXISTS") {
                        info!("topic {} already exists", topic);
                        break;
                    }
                    last_err = msg;
                    if attempt < 5 {
                        warn!(
                            "topic {} creation attempt {} failed, retrying...",
                            topic, attempt
                        );
                    }
                }
            }
        }
        if !last_err.is_empty() && attempt_topic_list().is_err() {
            warn!("topic {} may not exist: {}", topic, last_err);
        }
    }
    info!("topics ready");
    Ok(())
}

fn attempt_topic_list() -> Result<String> {
    docker_exec(RP_CONTAINER, &["rpk", "topic", "list"])
}

pub(crate) fn run_migrations() -> Result<()> {
    let database_url = "postgres://opsbucket:opsbucket@localhost:5432/opsbucket";

    info!("Running Postgres migrations...");
    let status = Command::new("cargo")
        .args(["run", "-p", "opsbucket-migrator", "--", "up"])
        .env("DATABASE_URL", &database_url)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .current_dir(SERVICES_DIR)
        .status()
        .context("migrations failed")?;
    if !status.success() {
        anyhow::bail!("migrations failed");
    }
    info!("migrations applied");
    Ok(())
}

pub(crate) fn flush_redis() -> Result<()> {
    info!("Flushing Redis...");
    docker_exec(REDIS_CONTAINER, &["redis-cli", "FLUSHALL"]).ok();
    info!("Redis flushed");
    Ok(())
}

pub(crate) fn seed_test_data() -> Result<()> {
    info!("Seeding test data...");
    crate::helpers::pg_query(
        "TRUNCATE write_keys CASCADE; TRUNCATE projects CASCADE; TRUNCATE identity_aliases CASCADE; \
         INSERT INTO projects (id, name) VALUES ('proj_test', 'Test Project'); \
         INSERT INTO write_keys (project_id, key) VALUES ('proj_test', 'wk_test_valid_key_12345');"
    )?;
    info!("seed data loaded");
    Ok(())
}

pub(crate) fn apply_clickhouse_schema() -> Result<()> {
    info!("Applying ClickHouse schema...");
    docker_exec_stdin(
        CH_CONTAINER,
        &["clickhouse-client", "--multiquery"],
        "DROP TABLE IF EXISTS events;",
    )?;
    let schema_path = format!("{}\\infra\\clickhouse\\schema.sql", crate::ROOT_DIR);
    let schema = std::fs::read_to_string(&schema_path).context("reading schema.sql")?;
    docker_exec_stdin(
        CH_CONTAINER,
        &["clickhouse-client", "--multiquery"],
        &schema,
    )?;
    info!("ClickHouse schema applied");
    Ok(())
}

// ── Phase 3: Build ────────────────────────────────────────────────

pub(crate) fn build_services() -> Result<()> {
    info!("Building services (this may take a while)...");
    for pkg in &[
        "opsbucket-ingestion",
        "opsbucket-processing",
        "opsbucket-query",
    ] {
        info!("building {}...", pkg);
        let status = Command::new("cargo")
            .args(["build", "-p", pkg])
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .current_dir(SERVICES_DIR)
            .status()
            .context(format!("building {}", pkg))?;
        if !status.success() {
            anyhow::bail!("failed to build {}", pkg);
        }
    }
    info!("all services built");
    Ok(())
}

// ── Phase 4: Start Services ───────────────────────────────────────

pub(crate) fn start_services() -> Result<(ServiceGuard, ServiceGuard, ServiceGuard)> {
    info!("Starting ingestion on port {}...", crate::INGEST_PORT);
    let ingestion = cargo_run(
        "opsbucket-ingestion",
        &[],
        &[
            ("KAFKA_BROKERS", "localhost:9092"),
            (
                "DATABASE_URL",
                "postgres://opsbucket:opsbucket@localhost:5432/opsbucket",
            ),
            ("REDIS_URL", "redis://localhost:6379"),
            ("RUST_LOG", "info"),
            ("PORT", &crate::INGEST_PORT.to_string()),
            ("RATE_LIMIT_CAPACITY", "2000"),
            ("RATE_LIMIT_REFILL", "2000"),
        ],
    )
    .context("starting ingestion")?;
    let ingest_guard = ServiceGuard::new(ingestion);

    info!("Starting processing consumer...");
    let processing = cargo_run(
        "opsbucket-processing",
        &[],
        &[
            ("KAFKA_BROKERS", "localhost:9092"),
            ("KAFKA_CONSUMER_GROUP", "opsbucket-e2e-processing"),
            (
                "DATABASE_URL",
                "postgres://opsbucket:opsbucket@localhost:5432/opsbucket",
            ),
            ("REDIS_URL", "redis://localhost:6379"),
            ("CLICKHOUSE_URL", "http://localhost:8123"),
            ("CLICKHOUSE_USER", "default"),
            ("CLICKHOUSE_PASSWORD", "opsbucket"),
            ("BATCH_SIZE", "100"),
            ("BATCH_TIMEOUT_MS", "3000"),
            ("RUST_LOG", "debug"),
        ],
    )
    .context("starting processing")?;
    let processing_guard = ServiceGuard::new(processing);

    info!("Starting query service on port {}...", crate::QUERY_PORT);
    let query = cargo_run(
        "opsbucket-query",
        &[],
        &[
            ("SECRET_KEY", crate::SECRET_KEY),
            (
                "DATABASE_URL",
                "postgres://opsbucket:opsbucket@localhost:5432/opsbucket",
            ),
            ("REDIS_URL", "redis://localhost:6379"),
            ("CLICKHOUSE_URL", "http://localhost:8123"),
            ("CLICKHOUSE_USER", "default"),
            ("CLICKHOUSE_PASSWORD", "opsbucket"),
            ("PORT", &crate::QUERY_PORT.to_string()),
            ("RUST_LOG", "info"),
        ],
    )
    .context("starting query service")?;
    let query_guard = ServiceGuard::new(query);

    Ok((ingest_guard, processing_guard, query_guard))
}
