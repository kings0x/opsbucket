use anyhow::Result;
use async_trait::async_trait;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

use super::PgHealth;

pub async fn init_pool(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .connect(database_url)
        .await?;
    Ok(pool)
}

pub async fn check_health(pool: &PgPool) -> Result<()> {
    sqlx::query("SELECT 1").execute(pool).await?;
    Ok(())
}

#[async_trait]
impl PgHealth for PgPool {
    async fn check_health(&self) -> Result<()> {
        check_health(self).await
    }
}

pub struct MockPgHealth;

#[async_trait]
impl PgHealth for MockPgHealth {
    async fn check_health(&self) -> Result<()> {
        Ok(())
    }
}
