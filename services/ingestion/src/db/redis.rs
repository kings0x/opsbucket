use anyhow::Result;
use async_trait::async_trait;
use deadpool_redis::{Config, Pool, Runtime};

use super::RedisHealth;

pub fn init_pool(redis_url: &str) -> Result<Pool> {
    let cfg = Config::from_url(redis_url);
    let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
    Ok(pool)
}

pub async fn check_health(pool: &Pool) -> Result<()> {
    let mut conn = pool.get().await?;
    let pong: String = redis::cmd("PING").query_async(&mut *conn).await?;
    if pong.to_uppercase() != "PONG" {
        anyhow::bail!("redis ping did not return PONG");
    }
    Ok(())
}

#[async_trait]
impl RedisHealth for Pool {
    async fn check_health(&self) -> Result<()> {
        check_health(self).await
    }
}

pub struct MockRedisHealth;

#[async_trait]
impl RedisHealth for MockRedisHealth {
    async fn check_health(&self) -> Result<()> {
        Ok(())
    }
}
