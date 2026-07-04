use base64::Engine;

use crate::Cursor;

pub fn build(
    project_id: &str,
    event_name: Option<&str>,
    user_id: Option<&str>,
    alias_anonymous_ids: &[String],
    cursor: Option<&Cursor>,
    limit: u64,
) -> String {
    let cursor_cond = match cursor {
        Some(c) => format!(
            "  AND (timestamp, event_id) < ('{}', '{}')",
            c.ts.format("%Y-%m-%d %H:%M:%S"),
            sql_escape(&c.id),
        ),
        None => String::new(),
    };

    let event_filter = match event_name {
        Some(name) => format!("event_name = '{}'", sql_escape(name)),
        None => "1 = 1".to_string(),
    };

    let user_filter = match user_id {
        Some(uid) => {
            let mut identities = vec![format!("'{}'", sql_escape(uid))];
            identities.extend(
                alias_anonymous_ids
                    .iter()
                    .map(|id| format!("'{}'", sql_escape(id))),
            );
            format!(
                "(user_id = '{}' OR anonymous_id IN ({}))",
                sql_escape(uid),
                identities.join(", "),
            )
        }
        None => "1 = 1".to_string(),
    };

    format!(
        "SELECT
    project_id, event_id, event_name, anonymous_id, user_id,
    timestamp, page_url, page_referrer, user_agent, properties
FROM events
WHERE project_id = '{}'
  AND ({})
  AND ({}){}
ORDER BY timestamp DESC, event_id DESC
LIMIT {}",
        sql_escape(project_id),
        event_filter,
        user_filter,
        cursor_cond,
        limit,
    )
}

fn sql_escape(s: &str) -> String {
    s.replace('\'', "\\'")
}

pub fn encode_cursor(ts: &chrono::DateTime<chrono::Utc>, id: &str) -> String {
    let cursor = Cursor {
        ts: *ts,
        id: id.to_string(),
    };
    let json = serde_json::to_string(&cursor).expect("cursor must serialize");
    base64::engine::general_purpose::STANDARD.encode(json)
}

pub fn decode_cursor(raw: &str) -> Result<Cursor, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(raw)
        .map_err(|e| format!("invalid cursor: {}", e))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("invalid cursor: {}", e))
}
