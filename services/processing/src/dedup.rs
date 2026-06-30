use anyhow::Result;
use opsbucket_shared::events::RawEvent;
use redis::aio::ConnectionManager;

pub async fn filter(
    batch: Vec<RawEvent>,
    redis: &mut ConnectionManager,
    ttl_seconds: u64,
) -> Result<Vec<RawEvent>> {
    let mut out = Vec::with_capacity(batch.len());
    for event in batch {
        let key = format!("seen:{}", event.message_id);
        let is_new: bool = redis::cmd("SETNX")
            .arg(&key)
            .arg("1")
            .query_async::<()>(redis)
            .await
            .map(|_: ()| true)
            .unwrap_or(false);
        if is_new {
            redis::cmd("EXPIRE")
                .arg(&key)
                .arg(ttl_seconds as i64)
                .query_async::<()>(redis)
                .await?;
            out.push(event);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use redis::aio::ConnectionManager;
    use redis::Client;

    async fn test_redis() -> ConnectionManager {
        let client = Client::open("redis://localhost:6379").unwrap();
        let mut conn = ConnectionManager::new(client).await.unwrap();
        redis::cmd("FLUSHDB")
            .query_async::<()>(&mut conn)
            .await
            .unwrap();
        conn
    }

    fn make_event(message_id: &str) -> RawEvent {
        RawEvent {
            project_id: "proj_1".into(),
            received_at: "2026-06-30T10:00:00Z".into(),
            sent_at: "2026-06-30T10:00:00Z".into(),
            ip: "127.0.0.1".into(),
            message_id: message_id.into(),
            event_type: "track".into(),
            anonymous_id: "anon_1".into(),
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
            properties: Some(serde_json::json!({"button_text": "Sign Up"})),
            traits: None,
            name: None,
        }
    }

    #[tokio::test]
    async fn first_occurrence_is_kept() {
        let mut redis = test_redis().await;
        let events = vec![make_event("msg-1")];
        let result = filter(events, &mut redis, 3600).await.unwrap();
        assert_eq!(result.len(), 1);
    }

    #[tokio::test]
    async fn duplicate_within_ttl_is_dropped() {
        let mut redis = test_redis().await;
        let e = make_event("msg-1");
        let first = filter(vec![e.clone()], &mut redis, 3600).await.unwrap();
        assert_eq!(first.len(), 1);
        let second = filter(vec![e], &mut redis, 3600).await.unwrap();
        assert_eq!(second.len(), 0);
    }

    #[tokio::test]
    async fn different_ids_are_both_kept() {
        let mut redis = test_redis().await;
        let events = vec![make_event("msg-1"), make_event("msg-2")];
        let result = filter(events, &mut redis, 3600).await.unwrap();
        assert_eq!(result.len(), 2);
    }
}
