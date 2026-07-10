use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use sqlx::Row;
use tokio::sync::RwLock;
use tracing::info;

#[derive(Clone)]
pub struct SecretKeyStore {
    inner: Arc<RwLock<HashMap<String, Option<String>>>>,
    pg: PgPool,
    env_key: String,
}

impl SecretKeyStore {
    pub async fn new(pg: PgPool, env_key: String) -> Self {
        let mut map = HashMap::new();
        map.insert(env_key.clone(), None);
        let store = Self {
            inner: Arc::new(RwLock::new(map)),
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

    /// Check if a key is valid and authorized for the given project.
    /// - Global keys (None scope, including the env var) are authorized for any project.
    /// - Per-project keys are only authorized for their specific project.
    pub async fn check_key_for_project(&self, key: &str, project_id: &str) -> bool {
        let map = self.inner.read().await;
        match map.get(key) {
            None => false,
            Some(None) => true,
            Some(Some(scope)) => scope == project_id,
        }
    }

    async fn refresh(&self) {
        match self.load_db_keys().await {
            Ok(keys) => {
                let mut guard = self.inner.write().await;
                guard.clear();
                guard.insert(self.env_key.clone(), None);
                for (key, scope) in keys {
                    guard.insert(key, scope);
                }
                info!(count = guard.len(), "secret key store refreshed");
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to refresh secret keys from db");
            }
        }
    }

    async fn load_db_keys(&self) -> Result<Vec<(String, Option<String>)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT key, project_id FROM secret_keys WHERE revoked_at IS NULL ORDER BY created_at ASC",
        )
        .fetch_all(&self.pg)
        .await?;

        Ok(rows
            .iter()
            .map(|row| {
                let key: String = row.get(0);
                let pid: String = row.get(1);
                let scope = if pid.is_empty() { None } else { Some(pid) };
                (key, scope)
            })
            .collect())
    }
}

#[cfg(test)]
impl SecretKeyStore {
    /// Create a store for testing that uses an in-memory map instead of Postgres.
    /// Only available in test builds (behind `#[cfg(test)]`).
    pub fn mock_store(env_key: &str, keys: Vec<(&str, Option<&str>)>) -> Self {
        let mut map = HashMap::new();
        map.insert(env_key.to_string(), None);
        for (k, scope) in keys {
            map.insert(k.to_string(), scope.map(|s| s.to_string()));
        }
        SecretKeyStore {
            inner: Arc::new(RwLock::new(map)),
            pg: PgPool::connect_lazy("postgres://localhost/opsbucket").unwrap(),
            env_key: env_key.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store(env_key: &str, keys: Vec<(&str, Option<&str>)>) -> SecretKeyStore {
        let mut map = HashMap::new();
        map.insert(env_key.to_string(), None);
        for (k, scope) in keys {
            map.insert(k.to_string(), scope.map(|s| s.to_string()));
        }
        SecretKeyStore {
            inner: Arc::new(RwLock::new(map)),
            pg: PgPool::connect_lazy("postgres://localhost/opsbucket").unwrap(),
            env_key: env_key.to_string(),
        }
    }

    #[tokio::test]
    async fn global_key_authorized_for_any_project() {
        let store = make_store("sk_global", vec![]);
        assert!(store.check_key_for_project("sk_global", "proj_a").await);
        assert!(store.check_key_for_project("sk_global", "proj_b").await);
        assert!(
            store
                .check_key_for_project("sk_global", "any_project")
                .await
        );
    }

    #[tokio::test]
    async fn per_project_key_authorized_for_its_own_project() {
        let store = make_store("sk_admin", vec![("sk_proj_a", Some("proj_a"))]);
        assert!(store.check_key_for_project("sk_proj_a", "proj_a").await);
    }

    #[tokio::test]
    async fn per_project_key_rejected_for_different_project() {
        let store = make_store("sk_admin", vec![("sk_proj_a", Some("proj_a"))]);
        assert!(!store.check_key_for_project("sk_proj_a", "proj_b").await);
    }

    #[tokio::test]
    async fn per_project_key_rejected_for_other_project() {
        let store = make_store("sk_admin", vec![("sk_proj_a", Some("proj_a"))]);
        assert!(!store.check_key_for_project("sk_proj_a", "proj_b").await);
        assert!(!store.check_key_for_project("sk_proj_b", "proj_a").await);
    }

    #[tokio::test]
    async fn unknown_key_rejected() {
        let store = make_store("sk_admin", vec![]);
        assert!(!store.check_key_for_project("unknown_key", "proj_a").await);
    }

    #[tokio::test]
    async fn empty_scope_key_treated_as_global() {
        let store = make_store("sk_admin", vec![("sk_empty", None)]);
        assert!(store.check_key_for_project("sk_empty", "proj_a").await);
        assert!(store.check_key_for_project("sk_empty", "proj_b").await);
    }

    #[tokio::test]
    async fn multiple_per_project_keys_scoped_correctly() {
        let store = make_store(
            "sk_admin",
            vec![
                ("sk_proj_a", Some("proj_a")),
                ("sk_proj_b", Some("proj_b")),
                ("sk_proj_c", Some("proj_c")),
            ],
        );
        assert!(store.check_key_for_project("sk_proj_a", "proj_a").await);
        assert!(store.check_key_for_project("sk_proj_b", "proj_b").await);
        assert!(store.check_key_for_project("sk_proj_c", "proj_c").await);
        assert!(!store.check_key_for_project("sk_proj_a", "proj_c").await);
        assert!(!store.check_key_for_project("sk_proj_b", "proj_a").await);
    }

    #[tokio::test]
    async fn env_key_is_always_global() {
        let store = make_store("my-env-key", vec![("sk_proj_a", Some("proj_a"))]);
        assert!(store.check_key_for_project("my-env-key", "proj_a").await);
        assert!(store.check_key_for_project("my-env-key", "proj_b").await);
        assert!(store.check_key_for_project("my-env-key", "proj_c").await);
    }
}
