use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};

mod config;
mod install;
mod manage;
mod status;
mod templates;
mod update;

#[derive(Parser)]
#[command(name = "opsbucket", about = "OpsBucket self-hosted analytics platform")]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// OpsBucket installation directory
    #[arg(short, long, default_value = "./opsbucket")]
    dir: PathBuf,
}

#[derive(Subcommand)]
enum Command {
    /// Install OpsBucket on this machine
    Install {
        /// Domain name (e.g., opsbucket.example.com)
        #[arg(short, long)]
        domain: String,

        /// Admin email address
        #[arg(short, long)]
        email: String,

        /// Admin password (auto-generated if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Non-interactive mode (use all flags, no prompts)
        #[arg(long)]
        yes: bool,
    },
    /// Check status of all OpsBucket services
    Status,
    /// Update all OpsBucket services to the latest version
    Update,
    /// Tail logs for a service
    Logs {
        /// Service name (e.g., ingestion, processing, query)
        service: Option<String>,

        /// Follow log output
        #[arg(short, long)]
        follow: bool,

        /// Number of lines to show from the end
        #[arg(long)]
        tail: Option<usize>,
    },
    /// Restart a service
    Restart {
        /// Service name (e.g., ingestion, processing, query)
        service: String,
    },
    /// Stop and remove all OpsBucket services
    Uninstall {
        /// Keep volumes (data) when uninstalling
        #[arg(long)]
        keep_volumes: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cli = Cli::parse();
    let dir = std::fs::canonicalize(&cli.dir).unwrap_or(cli.dir.clone());

    match &cli.command {
        Command::Install {
            domain,
            email,
            password,
            yes: _,
        } => {
            let password = password.clone().unwrap_or_else(|| {
                use std::time::{SystemTime, UNIX_EPOCH};
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                format!("opsbucket-{:x}", nanos)
            });

            if !dir.exists() {
                std::fs::create_dir_all(&dir)
                    .context("failed to create installation directory")?;
            }

            let cfg = config::OpsBucketConfig::generate(
                &password,
                domain,
                email,
            );

            println!();
            println!("═══ OpsBucket Install ═══");
            println!();
            println!("  Domain:           {}", cfg.domain);
            println!("  Admin Email:       {}", cfg.admin_email);
            println!("  Admin Password:    {}", cfg.admin_password);
            println!("  Install Directory: {}", dir.display());
            println!();

            install::install(&cfg, &dir).await?;
        }

        Command::Status => {
            let cfg = config::OpsBucketConfig::load(&dir)?;
            status::status(&cfg, &dir).await?;
        }

        Command::Update => {
            let cfg = config::OpsBucketConfig::load(&dir)?;
            update::update(&cfg, &dir).await?;
        }

        Command::Logs {
            service,
            follow,
            tail,
        } => {
            let cfg = config::OpsBucketConfig::load(&dir)?;
            manage::logs(&cfg, &dir, service.as_deref(), *follow, *tail).await?;
        }

        Command::Restart { service } => {
            let cfg = config::OpsBucketConfig::load(&dir)?;
            manage::restart(&cfg, &dir, service).await?;
        }

        Command::Uninstall { keep_volumes } => {
            let cfg = config::OpsBucketConfig::load(&dir)?;
            manage::uninstall(&cfg, &dir, *keep_volumes).await?;
        }
    }

    Ok(())
}
