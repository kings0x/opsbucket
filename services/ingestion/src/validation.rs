use opsbucket_shared::events::{AnyEvent, BatchPayload};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub enum ValidationError {
    #[error("batch exceeds maximum of {max_events} events (got {got})")]
    BatchTooLarge { got: usize, max_events: usize },

    #[error("event {index}: {message}")]
    InvalidEvent { index: usize, message: String },
}

pub fn validate_batch(payload: &BatchPayload) -> Result<(), ValidationError> {
    let max_events: usize = 500;
    if payload.batch.len() > max_events {
        return Err(ValidationError::BatchTooLarge {
            got: payload.batch.len(),
            max_events,
        });
    }

    for (i, event) in payload.batch.iter().enumerate() {
        validate_event(event, i)?;
    }

    Ok(())
}

fn validate_event(event: &AnyEvent, index: usize) -> Result<(), ValidationError> {
    let err = |msg: &str| -> ValidationError {
        ValidationError::InvalidEvent {
            index,
            message: msg.to_string(),
        }
    };

    match event {
        AnyEvent::Track(e) => {
            if e.message_id.trim().is_empty() {
                return Err(err("messageId is required"));
            }
            if e.anonymous_id.trim().is_empty() {
                return Err(err("anonymousId is required"));
            }
            if e.event.trim().is_empty() {
                return Err(err("event is required for track events"));
            }
            if e.original_timestamp.trim().is_empty() {
                return Err(err("originalTimestamp is required"));
            }
            if !is_valid_iso8601(&e.original_timestamp) {
                return Err(err("originalTimestamp must be a valid ISO-8601 string"));
            }
        }
        AnyEvent::Identify(e) => {
            if e.message_id.trim().is_empty() {
                return Err(err("messageId is required"));
            }
            if e.anonymous_id.trim().is_empty() {
                return Err(err("anonymousId is required"));
            }
            if e.user_id.trim().is_empty() {
                return Err(err("userId is required for identify events"));
            }
            if e.original_timestamp.trim().is_empty() {
                return Err(err("originalTimestamp is required"));
            }
            if !is_valid_iso8601(&e.original_timestamp) {
                return Err(err("originalTimestamp must be a valid ISO-8601 string"));
            }
        }
        AnyEvent::Page(e) => {
            if e.message_id.trim().is_empty() {
                return Err(err("messageId is required"));
            }
            if e.anonymous_id.trim().is_empty() {
                return Err(err("anonymousId is required"));
            }
            if e.original_timestamp.trim().is_empty() {
                return Err(err("originalTimestamp is required"));
            }
            if !is_valid_iso8601(&e.original_timestamp) {
                return Err(err("originalTimestamp must be a valid ISO-8601 string"));
            }
        }
    }

    Ok(())
}

fn is_valid_iso8601(s: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(s).is_ok()
        || chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsbucket_shared::events::*;

    fn make_context() -> Context {
        Context {
            library: Library {
                name: "test".into(),
                version: "1.0".into(),
            },
            page: Page {
                url: "https://example.com".into(),
                path: "/".into(),
                referrer: "".into(),
                title: "Test".into(),
                search: "".into(),
            },
            screen: Screen {
                width: 1920,
                height: 1080,
                density: 1.0,
            },
            user_agent: "test".into(),
            locale: "en-US".into(),
            timezone: "UTC".into(),
            campaign: Campaign {
                source: None,
                medium: None,
                name: None,
                term: None,
                content: None,
            },
            ip: None,
        }
    }

    fn valid_track() -> AnyEvent {
        AnyEvent::Track(TrackEvent {
            message_id: "msg-1".into(),
            event_type: "track".into(),
            anonymous_id: "anon-1".into(),
            user_id: None,
            original_timestamp: "2026-06-29T10:00:00.000Z".into(),
            event: "Button Clicked".into(),
            properties: serde_json::json!({}),
            context: make_context(),
        })
    }

    fn valid_identify() -> AnyEvent {
        AnyEvent::Identify(IdentifyEvent {
            message_id: "msg-2".into(),
            event_type: "identify".into(),
            anonymous_id: "anon-2".into(),
            user_id: "user-1".into(),
            original_timestamp: "2026-06-29T10:00:00.000Z".into(),
            traits: serde_json::json!({}),
            context: make_context(),
        })
    }

    fn valid_page() -> AnyEvent {
        AnyEvent::Page(PageEvent {
            message_id: "msg-3".into(),
            event_type: "page".into(),
            anonymous_id: "anon-3".into(),
            user_id: None,
            original_timestamp: "2026-06-29T10:00:00.000Z".into(),
            name: Some("Home".into()),
            properties: serde_json::json!({}),
            context: make_context(),
        })
    }

    fn make_payload(events: Vec<AnyEvent>) -> BatchPayload {
        BatchPayload {
            sent_at: "2026-06-29T10:00:00.000Z".into(),
            batch: events,
        }
    }

    #[test]
    fn accepts_well_formed_batch() {
        let payload = make_payload(vec![valid_track(), valid_identify(), valid_page()]);
        assert!(validate_batch(&payload).is_ok());
    }

    #[test]
    fn rejects_batch_exceeding_500_events() {
        let events = vec![valid_track(); 501];
        let payload = make_payload(events);
        let err = validate_batch(&payload).unwrap_err();
        match err {
            ValidationError::BatchTooLarge { got, max_events } => {
                assert_eq!(got, 501);
                assert_eq!(max_events, 500);
            }
            _ => panic!("expected BatchTooLarge"),
        }
    }

    #[test]
    fn accepts_batch_of_exactly_500_events() {
        let events = vec![valid_track(); 500];
        let payload = make_payload(events);
        assert!(validate_batch(&payload).is_ok());
    }

    #[test]
    fn rejects_track_with_missing_event() {
        let mut event = match valid_track() {
            AnyEvent::Track(e) => e,
            _ => unreachable!(),
        };
        event.event = "".into();
        let payload = make_payload(vec![AnyEvent::Track(event)]);
        let err = validate_batch(&payload).unwrap_err();
        match err {
            ValidationError::InvalidEvent { index, message } => {
                assert_eq!(index, 0);
                assert!(message.contains("event"));
            }
            _ => panic!("expected InvalidEvent"),
        }
    }

    #[test]
    fn rejects_identify_with_missing_user_id() {
        let mut event = match valid_identify() {
            AnyEvent::Identify(e) => e,
            _ => unreachable!(),
        };
        event.user_id = "".into();
        let payload = make_payload(vec![AnyEvent::Identify(event)]);
        let err = validate_batch(&payload).unwrap_err();
        match err {
            ValidationError::InvalidEvent { index, message } => {
                assert_eq!(index, 0);
                assert!(message.contains("userId"));
            }
            _ => panic!("expected InvalidEvent"),
        }
    }

    #[test]
    fn rejects_event_with_missing_message_id() {
        let mut event = match valid_track() {
            AnyEvent::Track(e) => e,
            _ => unreachable!(),
        };
        event.message_id = "".into();
        let payload = make_payload(vec![AnyEvent::Track(event)]);
        let err = validate_batch(&payload).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidEvent { .. }));
    }

    #[test]
    fn rejects_event_with_missing_anonymous_id() {
        let mut event = match valid_track() {
            AnyEvent::Track(e) => e,
            _ => unreachable!(),
        };
        event.anonymous_id = "".into();
        let payload = make_payload(vec![AnyEvent::Track(event)]);
        let err = validate_batch(&payload).unwrap_err();
        assert!(matches!(err, ValidationError::InvalidEvent { .. }));
    }

    #[test]
    fn accepts_page_without_name() {
        let event = AnyEvent::Page(PageEvent {
            message_id: "msg-3".into(),
            event_type: "page".into(),
            anonymous_id: "anon-3".into(),
            user_id: None,
            original_timestamp: "2026-06-29T10:00:00.000Z".into(),
            name: None,
            properties: serde_json::json!({}),
            context: make_context(),
        });
        let payload = make_payload(vec![event]);
        assert!(validate_batch(&payload).is_ok());
    }

    #[test]
    fn accepts_track_without_user_id() {
        let event = valid_track();
        let payload = make_payload(vec![event]);
        assert!(validate_batch(&payload).is_ok());
    }

    #[test]
    fn rejects_invalid_iso8601_timestamp() {
        let mut event = match valid_track() {
            AnyEvent::Track(e) => e,
            _ => unreachable!(),
        };
        event.original_timestamp = "not-a-timestamp".into();
        let payload = make_payload(vec![AnyEvent::Track(event)]);
        let err = validate_batch(&payload).unwrap_err();
        match err {
            ValidationError::InvalidEvent { index, message } => {
                assert_eq!(index, 0);
                assert!(message.contains("originalTimestamp"));
            }
            _ => panic!("expected InvalidEvent"),
        }
    }

    #[test]
    fn correct_index_in_multiple_events() {
        let mut bad = valid_track();
        if let AnyEvent::Track(ref mut e) = bad {
            e.event = "".into();
        }
        let payload = make_payload(vec![valid_track(), bad, valid_identify()]);
        let err = validate_batch(&payload).unwrap_err();
        match err {
            ValidationError::InvalidEvent { index, .. } => {
                assert_eq!(index, 1);
            }
            _ => panic!("expected InvalidEvent at index 1"),
        }
    }
}
