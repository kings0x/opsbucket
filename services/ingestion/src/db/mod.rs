pub mod postgres;
pub mod redis;

pub use postgres::MockPgHealth;
pub use redis::MockRedisHealth;

use async_trait::async_trait;

#[async_trait]
pub trait PgHealth: Send + Sync {
    async fn check_health(&self) -> anyhow::Result<()>;
}

#[async_trait]
pub trait RedisHealth: Send + Sync {
    async fn check_health(&self) -> anyhow::Result<()>;
}
