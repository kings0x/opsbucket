use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Clone)]
pub struct SecretKeyStore {
    inner: Arc<RwLock<Vec<String>>>,
    pg: PgPool,
    env_key: String,
}

impl SecretKeyStore {
    pub async fn new(pg: PgPool, env_key: String) -> Self {
        let store = Self {
            inner: Arc::new(RwLock::new(vec![env_key.clone()])),
            pg,
            env_key,
        };
        store.refresh().await;
        store
    }

    pub fn start_background_refresh(&self) {
        let this = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(60)).await;
                this.refresh().await;
            }
        });
    }

    pub async fn contains(&self, candidate: &str) -> bool {
        let keys = self.inner.read().await;
        keys.iter().any(|k| k == candidate)
    }

    async fn refresh(&self) {
        match self.load_db_keys().await {
            Ok(keys) => {
                let mut guard = self.inner.write().await;
                *guard = keys;
                info!(count = guard.len(), "secret key store refreshed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to refresh secret keys from db");
            }
        }
    }

    async fn load_db_keys(&self) -> Result<Vec<String>, sqlx::Error> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT key FROM secret_keys WHERE revoked_at IS NULL ORDER BY created_at ASC",
        )
        .fetch_all(&self.pg)
        .await?;

        let mut keys: Vec<String> = rows.into_iter().map(|r| r.0).collect();
        if !keys.contains(&self.env_key) {
            keys.push(self.env_key.clone());
        }
        Ok(keys)
    }
}
