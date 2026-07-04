use anyhow::Result;
use opsbucket_shared::events::RawEvent;
use redis::aio::ConnectionManager;
use sqlx::PgPool;

pub async fn resolve(
    batch: Vec<RawEvent>,
    redis: &mut ConnectionManager,
    pg: &PgPool,
    cache_ttl: u64,
) -> Result<Vec<(RawEvent, Option<String>)>> {
    let mut out = Vec::with_capacity(batch.len());
    for event in batch {
        let resolved_user_id = if let Some(uid) = &event.user_id {
            Some(uid.clone())
        } else {
            lookup_alias(&event.project_id, &event.anonymous_id, redis, pg, cache_ttl).await?
        };
        out.push((event, resolved_user_id));
    }
    Ok(out)
}

async fn lookup_alias(
    project_id: &str,
    anonymous_id: &str,
    redis: &mut ConnectionManager,
    pg: &PgPool,
    cache_ttl: u64,
) -> Result<Option<String>> {
    let cache_key = format!("alias:{}:{}", project_id, anonymous_id);

    let cached: Option<String> = redis::cmd("GET")
        .arg(&cache_key)
        .query_async::<Option<String>>(redis)
        .await?;
    if let Some(uid) = cached {
        return Ok(Some(uid));
    }

    let row: Option<(String,)> = sqlx::query_as(
        "SELECT user_id FROM identity_aliases WHERE project_id = $1 AND anonymous_id = $2",
    )
    .bind(project_id)
    .bind(anonymous_id)
    .fetch_optional(pg)
    .await?;

    if let Some((user_id,)) = row {
        redis::cmd("SETEX")
            .arg(&cache_key)
            .arg(cache_ttl as i64)
            .arg(&user_id)
            .query_async::<()>(redis)
            .await?;
        Ok(Some(user_id))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use redis::Client;
    use sqlx::postgres::PgPoolOptions;

    const PG_URL: &str = "postgres://opsbucket:opsbucket@127.0.0.1:5432/opsbucket";
    const REDIS_URL: &str = "redis://127.0.0.1:6379";

    fn make_event(anonymous_id: &str) -> RawEvent {
        RawEvent {
            project_id: "proj_1".into(),
            received_at: "2026-06-30T10:00:00Z".into(),
            sent_at: "2026-06-30T10:00:00Z".into(),
            ip: "127.0.0.1".into(),
            message_id: "msg-1".into(),
            event_type: "track".into(),
            anonymous_id: anonymous_id.into(),
            user_id: None,
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
            event: Some("Button Clicked".into()),
            properties: Some(serde_json::json!({})),
            traits: None,
            name: None,
        }
    }

    #[tokio::test]
    async fn event_with_user_id_skips_lookup() {
        let mut event = make_event("anon_1");
        event.user_id = Some("usr_1".into());

        let client = Client::open(REDIS_URL).unwrap();
        let mut redis = ConnectionManager::new(client).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut redis)
            .await
            .unwrap();

        let pg = PgPoolOptions::new()
            .max_connections(5)
            .connect(PG_URL)
            .await
            .unwrap();
        sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_1'")
            .execute(&pg)
            .await
            .unwrap();

        let result = resolve(vec![event], &mut redis, &pg, 600).await.unwrap();
        assert_eq!(result[0].1, Some("usr_1".to_string()));
    }

    #[tokio::test]
    async fn no_alias_returns_none() {
        let event = make_event("anon_unknown");

        let client = Client::open(REDIS_URL).unwrap();
        let mut redis = ConnectionManager::new(client).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut redis)
            .await
            .unwrap();

        let pg = PgPoolOptions::new()
            .max_connections(5)
            .connect(PG_URL)
            .await
            .unwrap();
        sqlx::query("DELETE FROM identity_aliases WHERE project_id = 'proj_1'")
            .execute(&pg)
            .await
            .unwrap();

        let result = resolve(vec![event], &mut redis, &pg, 600).await.unwrap();
        assert_eq!(result[0].1, None);
    }
}
