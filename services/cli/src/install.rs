use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::OpsBucketConfig;
use crate::templates;

pub async fn install(
    cfg: &OpsBucketConfig,
    dir: &Path,
) -> Result<()> {
    info!("installing OpsBucket to {}", dir.display());

    // 1. Write config files
    info!("writing configuration files...");
    cfg.save(dir)?;
    let compose = templates::generate_docker_compose(cfg);
    std::fs::write(dir.join("docker-compose.yml"), &compose)
        .context("failed to write docker-compose.yml")?;
    let caddyfile = templates::generate_caddyfile(cfg);
    std::fs::write(dir.join("Caddyfile"), &caddyfile)
        .context("failed to write Caddyfile")?;
    let schema = templates::generate_clickhouse_schema();
    std::fs::write(dir.join("clickhouse-schema.sql"), schema)
        .context("failed to write clickhouse-schema.sql")?;
    let topics = templates::generate_topic_script();
    std::fs::write(dir.join("topics.sh"), topics)
        .context("failed to write topics.sh")?;

    // 2. Pull images
    info!("pulling Docker images...");
    run_compose(cfg, dir, &["pull"]).await?;

    // 3. Start infra services
    info!("starting infrastructure services...");
    run_compose(
        cfg,
        dir,
        &["up", "-d", "postgres", "redis", "redpanda", "clickhouse", "minio"],
    )
    .await?;

    // 4. Wait for infra health
    info!("waiting for infrastructure to be healthy...");
    wait_for_healthy(cfg, dir, &["postgres", "redis", "redpanda", "clickhouse", "minio"]).await?;

    // 5. Create Kafka topics
    info!("creating Kafka topics...");
    docker_exec(
        cfg,
        dir,
        "redpanda",
        &[
            "rpk",
            "topic",
            "create",
            "raw-events",
            "--partitions",
            "12",
        ],
    )
    .await?;
    docker_exec(
        cfg,
        dir,
        "redpanda",
        &[
            "rpk",
            "topic",
            "create",
            "raw-events-dlq",
            "--partitions",
            "1",
        ],
    )
    .await?;

    // 6. Run migrations
    info!("running database migrations...");
    run_compose(cfg, dir, &["up", "migrator"]).await?;
    wait_for_container_exit(cfg, dir, "opsbucket-migrator").await?;

    // 7. Start pipeline services
    info!("starting pipeline services...");
    run_compose(
        cfg,
        dir,
        &[
            "up",
            "-d",
            "ingestion",
            "processing",
            "archiver",
            "query",
            "dashboard",
            "caddy",
        ],
    )
    .await?;

    // 8. Wait for dashboard
    info!("waiting for dashboard to be ready...");
    let dashboard_url = format!("http://127.0.0.1:8082/api/admin/health");
    wait_for_http(&dashboard_url, Duration::from_secs(60)).await?;

    // 9. Create admin user
    info!("creating admin user...");
    let setup_payload = serde_json::json!({
        "email": cfg.admin_email,
        "password": cfg.admin_password,
    });
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://127.0.0.1:8082/api/admin/setup"))
        .json(&setup_payload)
        .send()
        .await
        .context("failed to create admin user")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("admin setup failed ({}): {}", status, body);
    }

    let setup_result: serde_json::Value = resp
        .json()
        .await
        .context("failed to parse admin setup response")?;
    let admin_token = setup_result["token"]
        .as_str()
        .context("missing token in setup response")?
        .to_string();
    info!("admin user created");

    // 10. Create default project + write key
    info!("creating default project...");
    let project_payload = serde_json::json!({
        "name": "Default Project",
    });
    let resp = client
        .post(format!("http://127.0.0.1:8082/api/admin/projects"))
        .header("Authorization", format!("Bearer {}", admin_token))
        .json(&project_payload)
        .send()
        .await
        .context("failed to create default project")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("project creation failed ({}): {}", status, body);
    }

    let project_result: serde_json::Value = resp
        .json()
        .await
        .context("failed to parse project response")?;
    let project_id = project_result["id"]
        .as_str()
        .context("missing project id")?
        .to_string();
    let write_key = project_result["write_key"]
        .as_str()
        .context("missing write key")?
        .to_string();

    // 11. Print success
    let protocol = if cfg.domain.contains("localhost") || cfg.domain.contains("127.0.0.1") {
        "http"
    } else {
        "https"
    };

    println!();
    println!("═══════════════════════════════════════");
    println!("  OpsBucket is now running!");
    println!("═══════════════════════════════════════");
    println!();
    println!("  Dashboard:    {}://{}/", protocol, cfg.domain);
    println!("  Admin Email:  {}", cfg.admin_email);
    println!("  Admin Password: {}", cfg.admin_password);
    println!();
    println!("  SDK Configuration:");
    println!("    Write Key:   {}", write_key);
    println!("    Project ID:  {}", project_id);
    println!("    Endpoint:    {}://api.{}/v1/batch", protocol, cfg.domain);
    println!();
    println!("  Query API Secret Key: {}", cfg.secret_key);
    println!();
    println!("  Installation directory: {}", dir.display());
    println!("═══════════════════════════════════════");
    println!();

    let summary = format!(
        "Dashboard: {protocol}://{domain}/\n\
         Admin Email: {email}\n\
         Admin Password: {password}\n\
         Write Key: {wk}\n\
         Project ID: {pid}\n\
         Secret Key: {sk}\n\
         Dir: {dir}\n",
        protocol = protocol,
        domain = cfg.domain,
        email = cfg.admin_email,
        password = cfg.admin_password,
        wk = write_key,
        pid = project_id,
        sk = cfg.secret_key,
        dir = dir.display(),
    );
    std::fs::write(dir.join("setup.txt"), &summary).ok();

    info!("installation complete");
    Ok(())
}

async fn run_compose(
    cfg: &OpsBucketConfig,
    dir: &Path,
    args: &[&str],
) -> Result<()> {
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

async fn wait_for_healthy(
    _cfg: &OpsBucketConfig,
    _dir: &Path,
    services: &[&str],
) -> Result<()> {
    for service in services {
        let container = format!("opsbucket-{}", service);
        for _ in 0..30 {
            let output = tokio::process::Command::new("docker")
                .arg("inspect")
                .arg("--format")
                .arg("{{.State.Health.Status}}")
                .arg(&container)
                .output()
                .await
                .ok();

            if let Some(output) = output {
                let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if status == "healthy" {
                    info!("{} is healthy", service);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
    }
    Ok(())
}

async fn wait_for_container_exit(
    _cfg: &OpsBucketConfig,
    _dir: &Path,
    container: &str,
) -> Result<()> {
    for _ in 0..60 {
        let output = tokio::process::Command::new("docker")
            .arg("inspect")
            .arg("--format")
            .arg("{{.State.Status}}")
            .arg(container)
            .output()
            .await
            .ok();

        match output {
            Some(o) => {
                let status = String::from_utf8_lossy(&o.stdout).trim().to_string();
                if status == "exited" {
                    // Check exit code
                    let code_output = tokio::process::Command::new("docker")
                        .arg("inspect")
                        .arg("--format")
                        .arg("{{.State.ExitCode}}")
                        .arg(container)
                        .output()
                        .await
                        .ok();
                    if let Some(c) = code_output {
                        let code = String::from_utf8_lossy(&c.stdout).trim().to_string();
                        if code == "0" {
                            info!("{} completed successfully", container);
                            return Ok(());
                        } else {
                            // Show logs on failure
                            let _ = tokio::process::Command::new("docker")
                                .arg("logs")
                                .arg(container)
                                .stdout(std::process::Stdio::inherit())
                                .stderr(std::process::Stdio::inherit())
                                .spawn();
                            anyhow::bail!("{} exited with code {}", container, code);
                        }
                    }
                    return Ok(());
                }
                if status == "running" {
                    // Container is still running (migrator may not have exited yet)
                }
            }
            None => {
                // Container not found yet
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    anyhow::bail!("{} did not complete within timeout", container);
}

async fn docker_exec(
    _cfg: &OpsBucketConfig,
    _dir: &Path,
    service: &str,
    args: &[&str],
) -> Result<()> {
    let container = format!("opsbucket-{}", service);

    let mut cmd = tokio::process::Command::new("docker");
    cmd.arg("exec");
    if args.len() > 1 {
        // Check if we need -T flag for non-interactive (stdin not available)
        cmd.arg(container);
        cmd.args(args);
    } else {
        cmd.arg(container);
        cmd.args(args);
    }

    let output = cmd.output().await.context("failed to execute docker exec")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::warn!("docker exec {} {}: {}", service, args.join(" "), stderr);
    }

    Ok(())
}

async fn wait_for_http(url: &str, timeout: Duration) -> Result<()> {
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Ok(resp) = client.get(url).send().await {
            if resp.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    anyhow::bail!("{} did not become ready within {:?}", url, timeout);
}
