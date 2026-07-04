use anyhow::Result;
use opsbucket_shared::events::RawEvent;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

pub async fn handle_identify(
    event: &RawEvent,
    pg: &PgPool,
    redis: &mut ConnectionManager,
) -> Result<()> {
    let user_id = event.user_id.as_deref().unwrap_or("");
    sqlx::query(
        "INSERT INTO identity_aliases (project_id, anonymous_id, user_id, created_at)
         VALUES ($1, $2, $3, now())
         ON CONFLICT (project_id, anonymous_id) DO UPDATE SET user_id = $3",
    )
    .bind(&event.project_id)
    .bind(&event.anonymous_id)
    .bind(user_id)
    .execute(pg)
    .await?;

    let cache_key = format!("alias:{}:{}", event.project_id, event.anonymous_id);
    redis::cmd("DEL")
        .arg(&cache_key)
        .query_async::<()>(redis)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use redis::Client;

    fn make_identify_event(anonymous_id: &str, user_id: &str) -> RawEvent {
        RawEvent {
            project_id: "proj_1".into(),
            received_at: "2026-06-30T10:00:00Z".into(),
            sent_at: "2026-06-30T10:00:00Z".into(),
            ip: "127.0.0.1".into(),
            message_id: "msg-id-1".into(),
            event_type: "identify".into(),
            anonymous_id: anonymous_id.into(),
            user_id: Some(user_id.into()),
            original_timestamp: "2026-06-30T09:59:58Z".into(),
            context: opsbucket_shared::events::Context {
                library: opsbucket_shared::events::Library {
                    name: "@opsbucket/browser".into(),
                    version: "0.1.0".into(),
                },
                page: opsbucket_shared::events::Page {
                    url: "https://example.com".into(),
                    path: "/".into(),
                    referrer: "".into(),
                    title: "Home".into(),
                    search: "".into(),
                },
                screen: opsbucket_shared::events::Screen {
                    width: 1440,
                    height: 900,
                    density: 2.0,
                },
                user_agent: "test-agent".into(),
                locale: "en-US".into(),
                timezone: "UTC".into(),
                campaign: opsbucket_shared::events::Campaign {
                    source: None,
                    medium: None,
                    name: None,
                    term: None,
                    content: None,
                },
                ip: None,
            },
            event: None,
            properties: None,
            traits: Some(serde_json::json!({"email": "jane@example.com"})),
            name: None,
        }
    }

    #[tokio::test]
    async fn identify_upserts_alias_row() {
        let pg = PgPool::connect("postgres://opsbucket:opsbucket@localhost:5432/opsbucket")
            .await
            .unwrap();
        sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_1'")
            .execute(&pg)
            .await
            .unwrap();

        let client = Client::open("redis://localhost:6379").unwrap();
        let mut redis = ConnectionManager::new(client).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut redis)
            .await
            .unwrap();

        let event = make_identify_event("anon_upsert", "usr_1");
        handle_identify(&event, &pg, &mut redis).await.unwrap();

        let row: (String,) = sqlx::query_as(
            "SELECT user_id FROM identity_aliases WHERE project_id = $1 AND anonymous_id = $2",
        )
        .bind("proj_1")
        .bind("anon_upsert")
        .fetch_one(&pg)
        .await
        .unwrap();
        assert_eq!(row.0, "usr_1");
    }

    #[tokio::test]
    async fn re_identify_updates_existing_row() {
        let pg = PgPool::connect("postgres://opsbucket:opsbucket@localhost:5432/opsbucket")
            .await
            .unwrap();
        sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_1'")
            .execute(&pg)
            .await
            .unwrap();

        let client = Client::open("redis://localhost:6379").unwrap();
        let mut redis = ConnectionManager::new(client).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut redis)
            .await
            .unwrap();

        let e1 = make_identify_event("anon_re_id", "usr_old");
        handle_identify(&e1, &pg, &mut redis).await.unwrap();

        let e2 = make_identify_event("anon_re_id", "usr_new");
        handle_identify(&e2, &pg, &mut redis).await.unwrap();

        let row: (String,) = sqlx::query_as(
            "SELECT user_id FROM identity_aliases WHERE project_id = $1 AND anonymous_id = $2",
        )
        .bind("proj_1")
        .bind("anon_re_id")
        .fetch_one(&pg)
        .await
        .unwrap();
        assert_eq!(row.0, "usr_new");
    }

    #[tokio::test]
    async fn identify_invalidates_cache() {
        let pg = PgPool::connect("postgres://opsbucket:opsbucket@localhost:5432/opsbucket")
            .await
            .unwrap();
        sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_1'")
            .execute(&pg)
            .await
            .unwrap();

        let client = Client::open("redis://localhost:6379").unwrap();
        let mut redis = ConnectionManager::new(client).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut redis)
            .await
            .unwrap();

        redis::cmd("SET")
            .arg("alias:proj_1:anon_cache_test")
            .arg("usr_stale")
            .query_async::<()>(&mut redis)
            .await
            .unwrap();

        let event = make_identify_event("anon_cache_test", "usr_fresh");
        handle_identify(&event, &pg, &mut redis).await.unwrap();

        let cached: Option<String> = redis::cmd("GET")
            .arg("alias:proj_1:anon_cache_test")
            .query_async(&mut redis)
            .await
            .unwrap();
        assert!(cached.is_none());
    }
}
