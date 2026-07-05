use base64::Engine;

use crate::ch::client::{QueryParam, QueryPlan};
use crate::Cursor;

pub fn build_plan(
    project_id: &str,
    event_name: Option<&str>,
    user_id: Option<&str>,
    alias_anonymous_ids: &[String],
    cursor: Option<&Cursor>,
    limit: u64,
) -> QueryPlan {
    let cursor_cond = if cursor.is_some() {
        "  AND (timestamp, event_id) < (?, ?)"
    } else {
        ""
    };

    let event_filter = if event_name.is_some() {
        "event_name = ?"
    } else {
        "1 = 1"
    };

    let user_filter = if user_id.is_some() {
        let anonymous_placeholders = if alias_anonymous_ids.is_empty() {
            "''".to_string()
        } else {
            vec!["?"; alias_anonymous_ids.len()].join(", ")
        };
        format!(
            "(user_id = ? OR anonymous_id IN ({}))",
            anonymous_placeholders
        )
    } else {
        "1 = 1".to_string()
    };

    let sql = format!(
        "SELECT
    project_id, event_id, event_name, anonymous_id, user_id,
    timestamp, page_url, page_referrer, user_agent, properties
FROM events
WHERE project_id = ?
  AND ({})
  AND ({}){}
ORDER BY timestamp DESC, event_id DESC
LIMIT ?",
        event_filter, user_filter, cursor_cond,
    );

    let mut params = Vec::new();
    params.push(QueryParam::String(project_id.to_string()));
    if let Some(name) = event_name {
        params.push(QueryParam::String(name.to_string()));
    }
    if let Some(uid) = user_id {
        params.push(QueryParam::String(uid.to_string()));
        params.extend(alias_anonymous_ids.iter().cloned().map(QueryParam::String));
    }
    if let Some(c) = cursor {
        params.push(QueryParam::String(
            c.ts.format("%Y-%m-%d %H:%M:%S").to_string(),
        ));
        params.push(QueryParam::String(c.id.clone()));
    }
    params.push(QueryParam::U64(limit));

    QueryPlan::new(sql, params)
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
