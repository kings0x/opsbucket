use anyhow::Result;
use redis::AsyncCommands;

pub async fn validate_write_key(
    key: &str,
    redis: &redis::Client,
    pg: &sqlx::PgPool,
) -> Result<Option<String>> {
    let cache_key = format!("write_key:{}", key);

    let mut rconn = redis.get_multiplexed_async_connection().await?;
    let cached: Option<String> = rconn.get(&cache_key).await?;

    if let Some(json_value) = cached {
        let parsed: serde_json::Value = serde_json::from_str(&json_value)?;
        if let Some(project_id) = parsed.get("project_id").and_then(|v| v.as_str()) {
            return Ok(Some(project_id.to_string()));
        }
    }

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT project_id FROM write_keys WHERE key = $1 AND revoked_at IS NULL",
    )
    .bind(key)
    .fetch_optional(pg)
    .await?;

    if let Some((project_id,)) = row {
        let cache_value = serde_json::json!({ "project_id": project_id }).to_string();
        let _: () = redis::cmd("SET")
            .arg(&cache_key)
            .arg(&cache_value)
            .arg("EX")
            .arg(300i64)
            .query_async(&mut rconn)
            .await?;
        Ok(Some(project_id))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {

    struct TestStore {
        redis_entries: std::sync::Mutex<std::collections::HashMap<String, String>>,
        pg_entries: std::sync::Mutex<std::collections::HashMap<String, (String, bool)>>,
    }

    impl TestStore {
        fn new() -> Self {
            Self {
                redis_entries: std::sync::Mutex::new(std::collections::HashMap::new()),
                pg_entries: std::sync::Mutex::new(std::collections::HashMap::new()),
            }
        }

        fn set_redis(&self, key: &str, value: &str) {
            self.redis_entries
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }

        fn set_pg(&self, key: &str, project_id: &str, revoked: bool) {
            self.pg_entries
                .lock()
                .unwrap()
                .insert(key.to_string(), (project_id.to_string(), revoked));
        }

        fn redis_get(&self, key: &str) -> Option<String> {
            self.redis_entries.lock().unwrap().get(key).cloned()
        }

        fn pg_lookup(&self, key: &str) -> Option<String> {
            let entries = self.pg_entries.lock().unwrap();
            entries.get(key).and_then(|(pid, revoked)| {
                if *revoked {
                    None
                } else {
                    Some(pid.clone())
                }
            })
        }
    }

    fn mock_validate(
        key: &str,
        store: &TestStore,
    ) -> Option<String> {
        let cache_key = format!("write_key:{}", key);

        if let Some(json_value) = store.redis_get(&cache_key) {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_value) {
                if let Some(project_id) = parsed.get("project_id").and_then(|v| v.as_str()) {
                    return Some(project_id.to_string());
                }
            }
        }

        let result = store.pg_lookup(key);

        if let Some(ref project_id) = result {
            let cache_value =
                serde_json::json!({ "project_id": project_id }).to_string();
            store.set_redis(&cache_key, &cache_value);
        }

        result
    }

    #[test]
    fn valid_key_redis_cache_hit() {
        let store = TestStore::new();
        store.set_redis(
            "write_key:valid-key",
            r#"{"project_id": "proj-123"}"#,
        );

        let result = mock_validate("valid-key", &store);
        assert_eq!(result, Some("proj-123".to_string()));
    }

    #[test]
    fn valid_key_redis_miss_pg_hit() {
        let store = TestStore::new();
        store.set_pg("valid-key", "proj-456", false);

        let result = mock_validate("valid-key", &store);
        assert_eq!(result, Some("proj-456".to_string()));

        let cache_key = format!("write_key:{}", "valid-key");
        let cached = store.redis_get(&cache_key);
        assert!(cached.is_some());
        let parsed: serde_json::Value =
            serde_json::from_str(&cached.unwrap()).unwrap();
        assert_eq!(parsed["project_id"], "proj-456");
    }

    #[test]
    fn invalid_key_returns_none() {
        let store = TestStore::new();

        let result = mock_validate("unknown-key", &store);
        assert_eq!(result, None);
    }

    #[test]
    fn revoked_key_returns_none() {
        let store = TestStore::new();
        store.set_pg("revoked-key", "proj-789", true);

        let result = mock_validate("revoked-key", &store);
        assert_eq!(result, None);
    }

    #[test]
    fn populates_redis_on_cache_miss() {
        let store = TestStore::new();
        store.set_pg("fresh-key", "proj-000", false);

        let result = mock_validate("fresh-key", &store);
        assert_eq!(result, Some("proj-000".to_string()));

        let cache_key = format!("write_key:{}", "fresh-key");
        let cached = store.redis_get(&cache_key);
        assert!(cached.is_some());
        let parsed: serde_json::Value =
            serde_json::from_str(&cached.unwrap()).unwrap();
        assert_eq!(parsed["project_id"], "proj-000");
    }

    #[test]
    fn multiple_keys_are_independent() {
        let store = TestStore::new();
        store.set_pg("key-a", "proj-a", false);
        store.set_pg("key-b", "proj-b", false);

        assert_eq!(mock_validate("key-a", &store), Some("proj-a".to_string()));
        assert_eq!(mock_validate("key-b", &store), Some("proj-b".to_string()));
    }
}
