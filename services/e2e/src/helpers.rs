use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{json, Value};
use tokio::time::sleep;
use tracing::info;

use crate::{
    CLICKHOUSE_URL, HOST_LOOPBACK, INGEST_PORT, PG_CONTAINER, QUERY_PORT, SECRET_KEY, WRITE_KEY,
};

// ── Docker helpers ─────────────────────────────────────────────────

pub(crate) fn docker_exec(container: &str, cmd: &[&str]) -> Result<String> {
    let output = Command::new("docker")
        .arg("exec")
        .arg(container)
        .args(cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context(format!("docker exec {} {:?} failed", container, cmd))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker exec {} failed: {}", container, stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

pub(crate) fn docker_exec_stdin(container: &str, cmd: &[&str], stdin_data: &str) -> Result<String> {
    let mut child = Command::new("docker")
        .arg("exec")
        .arg("-i")
        .arg(container)
        .args(cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(format!("spawning docker exec {} {:?}", container, cmd))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data.as_bytes())?;
    }
    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("docker exec {} (stdin) failed: {}", container, stderr);
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

// ── Process helpers ────────────────────────────────────────────────

pub(crate) fn cargo_run(
    package: &str,
    _args: &[&str],
    envs: &[(&str, &str)],
) -> Result<std::process::Child> {
    let binary_path = format!("{}\\target\\debug\\{}.exe", crate::SERVICES_DIR, package);
    let mut cmd = Command::new(&binary_path);
    cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.spawn().context(format!("spawning {}", binary_path))
}

// ── Wait helpers ──────────────────────────────────────────────────

pub(crate) async fn wait_for_port(
    host: &str,
    port: u16,
    label: &str,
    timeout_secs: u64,
) -> Result<()> {
    info!("waiting for {} ({}:{})...", label, host, port);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;
    let url = format!("http://{}:{}", host, port);
    let start = Utc::now();
    loop {
        if (Utc::now() - start).num_seconds() > timeout_secs as i64 {
            anyhow::bail!("{} did not start within {}s", label, timeout_secs);
        }
        if client.get(&url).send().await.is_ok() {
            info!("{} is ready", label);
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn wait_for_events(expected: u64, label: &str, timeout_secs: u64) -> Result<()> {
    info!(
        "waiting for {} events in ClickHouse ({})...",
        expected, label
    );
    let start = Utc::now();
    loop {
        if (Utc::now() - start).num_seconds() > timeout_secs as i64 {
            anyhow::bail!("timeout waiting for {} events ({})", expected, label);
        }
        let count = ch_query("SELECT count() FROM events")
            .await
            .unwrap_or_default();
        if let Ok(n) = count.trim().parse::<u64>() {
            if n >= expected {
                info!("{} events present in ClickHouse", n);
                return Ok(());
            }
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn wait_for_event_id(event_id: &str, timeout_secs: u64) -> Result<()> {
    let start = Utc::now();
    loop {
        if (Utc::now() - start).num_seconds() > timeout_secs as i64 {
            anyhow::bail!("timeout waiting for event_id {}", event_id);
        }
        let count = ch_query(&format!(
            "SELECT count() FROM events WHERE event_id='{}'",
            event_id
        ))
        .await
        .unwrap_or_default();
        if count.trim() == "1" {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
}

pub(crate) async fn wait_for_container(
    container: &str,
    cmd: &[&str],
    label: &str,
    timeout_secs: u64,
) -> Result<()> {
    let start = Utc::now();
    loop {
        if (Utc::now() - start).num_seconds() > timeout_secs as i64 {
            anyhow::bail!("{} did not start within {}s", label, timeout_secs);
        }
        if docker_exec(container, cmd).is_ok() {
            return Ok(());
        }
        sleep(Duration::from_secs(2)).await;
    }
}

// ── Query helpers ─────────────────────────────────────────────────

pub(crate) async fn ch_query(sql: &str) -> Result<String> {
    let url = format!(
        "{}?user=default&password=opsbucket&readonly=0",
        CLICKHOUSE_URL
    );
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()?;
    let resp = client.post(&url).body(sql.to_string()).send().await?;
    Ok(resp.text().await?.trim().to_string())
}

pub(crate) fn pg_query(sql: &str) -> Result<String> {
    docker_exec(
        PG_CONTAINER,
        &[
            "psql",
            "-U",
            "opsbucket",
            "-d",
            "opsbucket",
            "-t",
            "-A",
            "-c",
            sql,
        ],
    )
}

pub(crate) async fn send_ingest(batch: &Value) -> Value {
    let client = reqwest::Client::new();
    let resp = match client
        .post(format!("http://{}:{}/v1/batch", HOST_LOOPBACK, INGEST_PORT))
        .header("Authorization", format!("Bearer {}", WRITE_KEY))
        .header("Content-Type", "application/json")
        .json(batch)
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return json!({}),
    };
    resp.json().await.unwrap_or(json!({}))
}

pub(crate) async fn send_ingest_raw(batch: &Value, key: &str) -> Result<Value> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}:{}/v1/batch", HOST_LOOPBACK, INGEST_PORT))
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(batch)
        .send()
        .await?;
    Ok(resp.json().await.unwrap_or(json!({})))
}

pub(crate) async fn query_service_get(path: &str) -> Result<(u16, Value)> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("http://{}:{}{}", HOST_LOOPBACK, QUERY_PORT, path))
        .header("Authorization", format!("Bearer {}", SECRET_KEY))
        .send()
        .await?;
    let status = resp.status().as_u16();
    let body = resp.json().await.unwrap_or(json!({}));
    Ok((status, body))
}

pub(crate) async fn query_service_post(path: &str, body: &Value) -> Result<(u16, Value)> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}:{}{}", HOST_LOOPBACK, QUERY_PORT, path))
        .header("Authorization", format!("Bearer {}", SECRET_KEY))
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await?;
    let status = resp.status().as_u16();
    let body_val = resp.json().await.unwrap_or(json!({}));
    Ok((status, body_val))
}

pub(crate) async fn query_service_post_status(path: &str, body: &Value, key: &str) -> Result<u16> {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("http://{}:{}{}", HOST_LOOPBACK, QUERY_PORT, path))
        .header("Authorization", format!("Bearer {}", key))
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await?;
    Ok(resp.status().as_u16())
}
