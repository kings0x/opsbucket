use std::path::Path;

use anyhow::Result;
use tracing::info;

use crate::config::OpsBucketConfig;

#[derive(Debug)]
pub struct ServiceStatus {
    pub name: String,
    pub status: String,
}

pub async fn status(_cfg: &OpsBucketConfig, _dir: &Path) -> Result<()> {
    info!("checking OpsBucket status...");

    let services = [
        "postgres",
        "redis",
        "redpanda",
        "clickhouse",
        "minio",
        "ingestion",
        "processing",
        "archiver",
        "query",
        "dashboard",
        "caddy",
    ];

    let mut results = Vec::new();

    for service in &services {
        let container = format!("opsbucket-{}", service);
        let output = tokio::process::Command::new("docker")
            .arg("inspect")
            .arg("--format")
            .arg("{{.State.Status}}|{{.State.Health.Status}}")
            .arg(&container)
            .output()
            .await
            .ok();

        let status = match output {
            Some(o) => {
                let raw = String::from_utf8_lossy(&o.stdout).trim().to_string();
                let parts: Vec<&str> = raw.split('|').collect();
                let state = parts.first().unwrap_or(&"unknown");
                let health = parts.get(1).unwrap_or(&"");

                match (*state, *health) {
                    ("running", "healthy") => "✅ healthy".into(),
                    ("running", "") => "✅ running".into(),
                    ("running", h) if !h.is_empty() => format!("⚠️  {}", h),
                    ("exited", _) => "❌ stopped".into(),
                    _ => format!("❌ {}", state),
                }
            }
            None => "❌ not found".into(),
        };

        results.push(ServiceStatus {
            name: service.to_string(),
            status,
        });
    }

    println!();
    println!("{:<20} {}", "Service", "Status");
    println!("{}", "─".repeat(40));
    for r in &results {
        println!("{:<20} {}", r.name, r.status);
    }
    println!();

    let client = reqwest::Client::new();
    match client
        .get("http://127.0.0.1:8082/api/admin/health")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(health) = resp.json::<serde_json::Value>().await {
                println!("Dashboard Health:");
                println!(
                    "  Status: {}",
                    health["status"].as_str().unwrap_or("unknown")
                );
                if let Some(checks) = health["checks"].as_object() {
                    for (svc, st) in checks {
                        let icon = if st == "ok" { "✅" } else { "❌" };
                        println!("  {} {}", icon, svc);
                    }
                }
            }
        }
        _ => {
            println!("Dashboard API: ❌ unreachable");
        }
    }
    println!();

    Ok(())
}
