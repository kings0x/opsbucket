use std::time::Duration;

use anyhow::{Context, Result};
use redis::aio::ConnectionManager;
use redis::Client as RedisClient;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

pub async fn init_pg(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url)
        .await
        .context("failed to connect to postgres")?;

    sqlx::query("SELECT 1")
        .execute(&pool)
        .await
        .context("postgres health check failed")?;

    tracing::info!("postgres connected");
    Ok(pool)
}

pub async fn init_redis(redis_url: &str) -> Result<ConnectionManager> {
    let client = RedisClient::open(redis_url).context("failed to parse redis url")?;
    let conn = ConnectionManager::new(client)
        .await
        .context("failed to connect to redis")?;

    let mut test = conn.clone();
    redis::cmd("PING")
        .query_async::<_, String>(&mut test)
        .await
        .context("redis health check failed")?;

    tracing::info!("redis connected");
    Ok(conn)
}
