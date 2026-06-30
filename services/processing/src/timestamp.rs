use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use opsbucket_shared::events::RawEvent;

#[derive(Debug, Clone)]
pub struct TimestampedEvent {
    pub event: RawEvent,
    pub resolved_user_id: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub fn correct(events: Vec<(RawEvent, Option<String>)>) -> Result<Vec<TimestampedEvent>> {
    events
        .into_iter()
        .map(|(event, resolved_user_id)| {
            let received_at =
                parse_dt(&event.received_at).context("failed to parse received_at")?;
            let original_timestamp = parse_dt(&event.original_timestamp)
                .context("failed to parse original_timestamp")?;
            let timestamp = if event.sent_at.is_empty() {
                tracing::warn!(
                    message_id = %event.message_id,
                    "sent_at is missing; falling back to received_at for timestamp"
                );
                received_at
            } else {
                match parse_dt(&event.sent_at) {
                    Ok(sent_at) => {
                        let skew = sent_at - original_timestamp;
                        received_at - skew
                    }
                    Err(e) => {
                        tracing::warn!(
                            message_id = %event.message_id,
                            error = %e,
                            "sent_at parse failed; falling back to received_at"
                        );
                        received_at
                    }
                }
            };
            Ok(TimestampedEvent {
                event,
                resolved_user_id,
                timestamp,
            })
        })
        .collect()
}

fn parse_dt(s: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ")
                .map(|n| n.and_utc())
                .map_err(anyhow::Error::from)
        })
        .context("invalid timestamp format")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(
        received_at: &str,
        sent_at: &str,
        original_timestamp: &str,
    ) -> (RawEvent, Option<String>) {
        let event = RawEvent {
            project_id: "proj_1".into(),
            received_at: received_at.into(),
            sent_at: sent_at.into(),
            ip: "127.0.0.1".into(),
            message_id: "msg-1".into(),
            event_type: "track".into(),
            anonymous_id: "anon_1".into(),
            user_id: None,
            original_timestamp: original_timestamp.into(),
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
        };
        (event, None)
    }

    #[test]
    fn correct_formula_with_clock_skew() {
        let event = make_event(
            "2026-06-30T10:00:30.000Z",
            "2026-06-30T10:00:05.000Z",
            "2026-06-30T10:00:03.000Z",
        );
        let result = correct(vec![event]).unwrap();
        let ts = result[0].timestamp;
        let expected: DateTime<Utc> = "2026-06-30T10:00:28.000Z".parse().unwrap();
        assert_eq!(ts, expected);
    }

    #[test]
    fn falls_back_when_sent_at_empty() {
        let event = make_event("2026-06-30T10:00:30.000Z", "", "2026-06-30T10:00:03.000Z");
        let result = correct(vec![event]).unwrap();
        let ts = result[0].timestamp;
        let expected: DateTime<Utc> = "2026-06-30T10:00:30.000Z".parse().unwrap();
        assert_eq!(ts, expected);
    }

    #[test]
    fn no_skew_when_sent_at_equals_original_timestamp() {
        let event = make_event(
            "2026-06-30T10:00:30.000Z",
            "2026-06-30T10:00:05.000Z",
            "2026-06-30T10:00:05.000Z",
        );
        let result = correct(vec![event]).unwrap();
        let ts = result[0].timestamp;
        let expected: DateTime<Utc> = "2026-06-30T10:00:30.000Z".parse().unwrap();
        assert_eq!(ts, expected);
    }
}
