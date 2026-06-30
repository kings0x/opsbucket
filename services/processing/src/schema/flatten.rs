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
        timestamp: event.timestamp,
        received_at: parse_dt(&event.event.received_at),
        original_timestamp: parse_dt(&event.event.original_timestamp),
        properties,
        traits,
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

fn build_properties(event: &TimestampedEvent) -> String {
    match event.event.event_type.as_str() {
        "track" => event
            .event
            .properties
            .as_ref()
            .map(stringify_map)
            .unwrap_or_else(|| "{}".to_string()),
        "identify" => event
            .event
            .traits
            .as_ref()
            .map(stringify_map)
            .unwrap_or_else(|| "{}".to_string()),
        "page" => {
            let mut merged = serde_json::Map::new();
            if let Some(props) = &event.event.properties {
                if let Some(obj) = props.as_object() {
                    for (k, v) in obj {
                        merged.insert(k.clone(), v.clone());
                    }
                }
            }
            merged.insert(
                "url".to_string(),
                serde_json::json!(event.event.context.page.url),
            );
            merged.insert(
                "path".to_string(),
                serde_json::json!(event.event.context.page.path),
            );
            merged.insert(
                "referrer".to_string(),
                serde_json::json!(event.event.context.page.referrer),
            );
            merged.insert(
                "title".to_string(),
                serde_json::json!(event.event.context.page.title),
            );
            merged.insert(
                "search".to_string(),
                serde_json::json!(event.event.context.page.search),
            );
            serde_json::Value::Object(merged).to_string()
        }
        _ => "{}".to_string(),
    }
}

fn build_traits(event: &TimestampedEvent) -> String {
    event
        .event
        .traits
        .as_ref()
        .map(stringify_map)
        .unwrap_or_else(|| "{}".to_string())
}

fn stringify_map(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                let s = json_value_to_string(v);
                out.insert(k.clone(), serde_json::Value::String(s));
            }
            serde_json::Value::Object(out).to_string()
        }
        _ => value.to_string(),
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
        let props: serde_json::Value = serde_json::from_str(&row.properties).unwrap();
        assert_eq!(props["button_text"], "Sign Up");
        assert_eq!(props["count"], "42");
        assert_eq!(props["active"], "true");
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
        let props: serde_json::Value = serde_json::from_str(&row.properties).unwrap();
        assert_eq!(props["email"], "jane@example.com");
        assert_eq!(props["plan"], "pro");
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
        let props: serde_json::Value = serde_json::from_str(&row.properties).unwrap();
        assert_eq!(props["url"], "https://example.com/page");
        assert_eq!(props["custom_prop"], "custom_val");
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
