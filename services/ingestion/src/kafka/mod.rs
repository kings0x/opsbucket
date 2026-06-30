pub mod producer;

use async_trait::async_trait;

#[async_trait]
pub trait KafkaHealth: Send + Sync {
    async fn check_health(&self) -> anyhow::Result<()>;
}
