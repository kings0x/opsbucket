pub mod write_key;

use async_trait::async_trait;

#[async_trait]
pub trait AuthValidator: Send + Sync {
    async fn validate(&self, key: &str) -> Option<String>;
}

pub struct RedisPgAuth {
    redis: redis::Client,
    pg: sqlx::PgPool,
}

impl RedisPgAuth {
    pub fn new(redis: redis::Client, pg: sqlx::PgPool) -> Self {
        Self { redis, pg }
    }
}

#[async_trait]
impl AuthValidator for RedisPgAuth {
    async fn validate(&self, key: &str) -> Option<String> {
        write_key::validate_write_key(key, &self.redis, &self.pg)
            .await
            .unwrap_or(None)
    }
}

pub struct MockAuth {
    keys: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl Default for MockAuth {
    fn default() -> Self {
        Self {
            keys: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

impl MockAuth {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, key: &str, project_id: &str) {
        self.keys
            .lock()
            .unwrap()
            .insert(key.to_string(), project_id.to_string());
    }
}

#[async_trait]
impl AuthValidator for MockAuth {
    async fn validate(&self, key: &str) -> Option<String> {
        self.keys.lock().unwrap().get(key).cloned()
    }
}
