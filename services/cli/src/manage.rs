use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::config::OpsBucketConfig;

pub async fn logs(
    cfg: &OpsBucketConfig,
    dir: &Path,
    service: Option<&str>,
    follow: bool,
    tail: Option<usize>,
) -> Result<()> {
    let compose_file = dir
        .join("docker-compose.yml")
        .to_str()
        .context("invalid path")?
        .to_string();

    let mut args = vec![
        "compose".to_string(),
        "-p".to_string(),
        cfg.compose_project.clone(),
        "-f".to_string(),
        compose_file,
        "logs".to_string(),
    ];

    if follow {
        args.push("-f".to_string());
    }
    if let Some(n) = tail {
        args.push("--tail".to_string());
        args.push(n.to_string());
    }
    if let Some(s) = service {
        args.push(s.to_string());
    }

    let mut cmd = tokio::process::Command::new("docker");
    cmd.args(&args);
    cmd.stdout(std::process::Stdio::inherit());
    cmd.stderr(std::process::Stdio::inherit());

    let status = cmd.status().await.context("failed to run docker logs")?;
    if !status.success() {
        anyhow::bail!("docker logs exited with code {:?}", status.code());
    }

    Ok(())
}

pub async fn restart(
    cfg: &OpsBucketConfig,
    dir: &Path,
    service: &str,
) -> Result<()> {
    info!("restarting {}...", service);
    let compose_file = dir.join("docker-compose.yml");
    let output = tokio::process::Command::new("docker")
        .arg("compose")
        .arg("-p")
        .arg(&cfg.compose_project)
        .arg("-f")
        .arg(&compose_file)
        .arg("restart")
        .arg(service)
        .output()
        .await
        .context("failed to restart service")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("restart {} failed: {}", service, stderr);
    }

    info!("{} restarted", service);
    Ok(())
}

pub async fn uninstall(
    cfg: &OpsBucketConfig,
    dir: &Path,
    keep_volumes: bool,
) -> Result<()> {
    info!("uninstalling OpsBucket...");

    let compose_file = dir.join("docker-compose.yml");
    let mut args = vec![
        "compose".to_string(),
        "-p".to_string(),
        cfg.compose_project.clone(),
        "-f".to_string(),
        compose_file.to_str().context("invalid path")?.to_string(),
        "down".to_string(),
    ];
    if !keep_volumes {
        args.push("-v".to_string());
    }

    let output = tokio::process::Command::new("docker")
        .args(&args)
        .output()
        .await
        .context("failed to stop services")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker compose down failed: {}", stderr);
    }

    info!("services stopped");
    Ok(())
}
