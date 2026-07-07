use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::OpsBucketConfig;

pub async fn update(cfg: &OpsBucketConfig, dir: &Path) -> Result<()> {
    info!("updating OpsBucket...");

    // 1. Pull latest images
    info!("pulling latest Docker images...");
    run_compose(cfg, dir, &["pull"]).await?;

    // 2. Recreate migrator and run migrations
    info!("running migrations...");
    run_compose(
        cfg,
        dir,
        &[
            "up",
            "-d",
            "postgres",
            "redis",
            "redpanda",
            "clickhouse",
            "minio",
        ],
    )
    .await?;
    run_compose(cfg, dir, &["up", "migrator"]).await?;

    // 3. Recreate all services
    info!("restarting pipeline services...");
    run_compose(cfg, dir, &["up", "-d", "--force-recreate", "--no-deps"]).await?;

    info!("update complete");
    Ok(())
}

async fn run_compose(cfg: &OpsBucketConfig, dir: &Path, args: &[&str]) -> Result<()> {
    let output = tokio::process::Command::new("docker")
        .arg("compose")
        .arg("-p")
        .arg(&cfg.compose_project)
        .arg("-f")
        .arg(dir.join("docker-compose.yml"))
        .args(args)
        .output()
        .await
        .context("failed to execute docker compose")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker compose {} failed: {}", args.join(" "), stderr);
    }

    Ok(())
}
