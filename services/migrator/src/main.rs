use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgPoolOptions;
use tracing::info;

static MIGRATOR: Migrator = sqlx::migrate!("../../infra/postgres/migrations");

#[derive(Parser)]
#[command(name = "migrator", about = "OpsBucket Postgres migration runner")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Apply all pending migrations
    Up,
    /// Mark a migration as applied without re-running it (recovery only)
    Force {
        /// Migration version number to mark as applied
        version: i64,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL must be set")?;

    let pool = PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&database_url)
        .await
        .context("failed to connect to postgres")?;

    info!("connected to postgres");

    match cli.command {
        Command::Up => {
            MIGRATOR.run(&pool).await?;
            info!("migrations up to date");
        }
        Command::Force { version } => {
            sqlx::query(
                r#"
                INSERT INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time)
                VALUES ($1, 'forced', NOW(), true, '\x00'::bytea, 0)
                ON CONFLICT (version) DO UPDATE SET success = true, installed_on = NOW()
                "#,
            )
            .bind(version)
            .execute(&pool)
            .await
            .context("failed to force migration version")?;

            info!("forced migration version {}", version);
        }
    }

    Ok(())
}
