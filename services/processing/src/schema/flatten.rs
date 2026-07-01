use std::collections::HashMap;

use crate::clickhouse::models::ClickHouseRow;
use crate::timestamp::TimestampedEvent;

pub fn flatten(event: TimestampedEvent) -> ClickHouseRow {
    let event_name = match event.event.event_type.as_str() {
        "track" => event.event.event.clone().unwrap_or_default(),
        "page" => "Page Viewed".to_string(),
        "identify" => "Identify".to_string(),
        other => other.to_string(),
    };

    let properties = build_properties(&event);
    let traits = build_traits(&event);

    let ctx = &event.event.context;

    ClickHouseRow {
        event_id: event.event.message_id.clone(),
        event_name,
        project_id: event.event.project_id.clone(),
        anonymous_id: event.event.anonymous_id.clone(),
        user_id: event.resolved_user_id.clone(),
        event_type: event.event.event_type.clone(),
        timestamp: event.timestamp.timestamp() as u32,
        received_at: parse_dt(&event.event.received_at).timestamp() as u32,
        original_timestamp: parse_dt(&event.event.original_timestamp).timestamp() as u32,
        properties: properties.into_iter().collect(),
        traits: traits.into_iter().collect(),
        user_agent: ctx.user_agent.clone(),
        locale: ctx.locale.clone(),
        timezone: ctx.timezone.clone(),
        ip: event.event.ip.clone(),
        library_name: ctx.library.name.clone(),
        library_version: ctx.library.version.clone(),
        page_url: ctx.page.url.clone(),
        page_path: ctx.page.path.clone(),
        page_referrer: ctx.page.referrer.clone(),
        page_title: ctx.page.title.clone(),
        page_search: ctx.page.search.clone(),
        screen_width: ctx.screen.width,
        screen_height: ctx.screen.height,
        screen_density: ctx.screen.density,
        campaign_source: ctx.campaign.source.clone(),
        campaign_medium: ctx.campaign.medium.clone(),
        campaign_name: ctx.campaign.name.clone(),
        campaign_term: ctx.campaign.term.clone(),
        campaign_content: ctx.campaign.content.clone(),
    }
}

fn build_properties(event: &TimestampedEvent) -> HashMap<String, String> {
    match event.event.event_type.as_str() {
        "track" => event
            .event
            .properties
            .as_ref()
            .map(stringify_map)
            .unwrap_or_default(),
        "identify" => event
            .event
            .traits
            .as_ref()
            .map(stringify_map)
            .unwrap_or_default(),
        "page" => {
            let mut merged = HashMap::new();
            if let Some(props) = &event.event.properties {
                if let Some(obj) = props.as_object() {
                    for (k, v) in obj {
                        merged.insert(k.clone(), json_value_to_string(v));
                    }
                }
            }
            merged.insert("url".to_string(), event.event.context.page.url.clone());
            merged.insert("path".to_string(), event.event.context.page.path.clone());
            merged.insert(
                "referrer".to_string(),
                event.event.context.page.referrer.clone(),
            );
            merged.insert("title".to_string(), event.event.context.page.title.clone());
            merged.insert(
                "search".to_string(),
                event.event.context.page.search.clone(),
            );
            merged
        }
        _ => HashMap::new(),
    }
}

fn build_traits(event: &TimestampedEvent) -> HashMap<String, String> {
    event
        .event
        .traits
        .as_ref()
        .map(stringify_map)
        .unwrap_or_default()
}

fn stringify_map(value: &serde_json::Value) -> HashMap<String, String> {
    match value {
        serde_json::Value::Object(map) => map
            .iter()
            .map(|(k, v)| (k.clone(), json_value_to_string(v)))
            .collect(),
        _ => HashMap::new(),
    }
}

fn json_value_to_string(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Array(arr) => {
            let strs: Vec<String> = arr.iter().map(json_value_to_string).collect();
            format!("[{}]", strs.join(","))
        }
        serde_json::Value::Object(_) => v.to_string(),
    }
}

fn parse_dt(s: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ").map(|n| n.and_utc())
        })
        .unwrap_or_else(|_| chrono::Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timestamp::correct;
    use opsbucket_shared::events::RawEvent;

    fn make_raw_event(
        event_type: &str,
        event: Option<&str>,
        properties: Option<serde_json::Value>,
        traits: Option<serde_json::Value>,
    ) -> (RawEvent, Option<String>) {
        let e = RawEvent {
            project_id: "proj_1".into(),
            received_at: "2026-06-30T10:00:30.000Z".into(),
            sent_at: "2026-06-30T10:00:05.000Z".into(),
            ip: "127.0.0.1".into(),
            message_id: "msg-1".into(),
            event_type: event_type.into(),
            anonymous_id: "anon_1".into(),
            user_id: None,
            original_timestamp: "2026-06-30T10:00:03.000Z".into(),
            context: opsbucket_shared::events::Context {
                library: opsbucket_shared::events::Library {
                    name: "@opsbucket/browser".into(),
                    version: "0.1.0".into(),
                },
                page: opsbucket_shared::events::Page {
                    url: "https://example.com/page".into(),
                    path: "/page".into(),
                    referrer: "https://google.com".into(),
                    title: "Test Page".into(),
                    search: "?q=test".into(),
                },
                screen: opsbucket_shared::events::Screen {
                    width: 1440,
                    height: 900,
                    density: 2.0,
                },
                user_agent: "Mozilla/5.0".into(),
                locale: "en-US".into(),
                timezone: "America/New_York".into(),
                campaign: opsbucket_shared::events::Campaign {
                    source: Some("google".into()),
                    medium: Some("cpc".into()),
                    name: Some("spring_sale".into()),
                    term: Some("analytics".into()),
                    content: Some("banner1".into()),
                },
                ip: Some("127.0.0.1".into()),
            },
            event: event.map(|s| s.to_string()),
            properties,
            traits,
            name: None,
        };
        (e, None)
    }

    #[test]
    fn track_event_name_matches_event_field() {
        let raw = make_raw_event(
            "track",
            Some("Button Clicked"),
            Some(serde_json::json!({"key": "val"})),
            None,
        );
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_eq!(row.event_name, "Button Clicked");
    }

    #[test]
    fn page_event_name_is_page_viewed() {
        let raw = make_raw_event(
            "page",
            None,
            Some(serde_json::json!({"url": "/home"})),
            None,
        );
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_eq!(row.event_name, "Page Viewed");
    }

    #[test]
    fn identify_event_name_is_identify() {
        let raw = make_raw_event(
            "identify",
            None,
            None,
            Some(serde_json::json!({"email": "j@e.com"})),
        );
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_eq!(row.event_name, "Identify");
    }

    fn assert_property(properties: &[(String, String)], key: &str, expected: &str) {
        let val = properties
            .iter()
            .find(|(k, _)| k == key)
            .unwrap_or_else(|| panic!("key {key} not found in properties"));
        assert_eq!(val.1, expected);
    }

    #[test]
    fn track_properties_are_preserved() {
        let raw = make_raw_event(
            "track",
            Some("Click"),
            Some(serde_json::json!({"button_text": "Sign Up", "count": 42, "active": true})),
            None,
        );
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_property(&row.properties, "button_text", "Sign Up");
        assert_property(&row.properties, "count", "42");
        assert_property(&row.properties, "active", "true");
    }

    #[test]
    fn identify_properties_from_traits() {
        let raw = make_raw_event(
            "identify",
            None,
            None,
            Some(serde_json::json!({"email": "jane@example.com", "plan": "pro"})),
        );
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_property(&row.properties, "email", "jane@example.com");
        assert_property(&row.properties, "plan", "pro");
    }

    #[test]
    fn page_properties_merge_with_context_page() {
        let raw = make_raw_event(
            "page",
            None,
            Some(serde_json::json!({"custom_prop": "custom_val"})),
            None,
        );
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_property(&row.properties, "url", "https://example.com/page");
        assert_property(&row.properties, "custom_prop", "custom_val");
    }

    #[test]
    fn camel_case_to_snake_case_rename() {
        let raw = make_raw_event("track", Some("Event"), Some(serde_json::json!({})), None);
        let ts = correct(vec![raw]).unwrap();
        let row = flatten(ts.into_iter().next().unwrap());
        assert_eq!(row.project_id, "proj_1");
        assert_eq!(row.event_id, "msg-1");
        assert_eq!(row.anonymous_id, "anon_1");
        assert_eq!(row.page_url, "https://example.com/page");
        assert_eq!(row.page_referrer, "https://google.com");
        assert_eq!(row.ip, "127.0.0.1");
        assert_eq!(row.user_agent, "Mozilla/5.0");
        assert_eq!(row.screen_width, 1440);
        assert_eq!(row.screen_height, 900);
        assert!((row.screen_density - 2.0).abs() < f32::EPSILON);
        assert_eq!(row.campaign_source, Some("google".to_string()));
        assert_eq!(row.campaign_medium, Some("cpc".to_string()));
        assert_eq!(row.locale, "en-US");
    }
}
