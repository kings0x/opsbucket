use std::collections::HashMap;

use sqlx::PgPool;

use crate::{EventRow, IdentityAlias};

pub async fn patch_identity(
    mut events: Vec<EventRow>,
    pg: &PgPool,
) -> Result<Vec<EventRow>, anyhow::Error> {
    let unresolved: Vec<&str> = events
        .iter()
        .filter(|e| e.user_id.is_none())
        .map(|e| e.anonymous_id.as_str())
        .collect();

    if unresolved.is_empty() {
        return Ok(events);
    }

    let aliases = sqlx::query_as::<_, IdentityAlias>(
        "SELECT project_id, anonymous_id, user_id
         FROM identity_aliases
         WHERE anonymous_id = ANY($1)",
    )
    .bind(&unresolved)
    .fetch_all(pg)
    .await?;

    let lookup: HashMap<&str, &str> = aliases
        .iter()
        .map(|a| (a.anonymous_id.as_str(), a.user_id.as_str()))
        .collect();

    for event in events.iter_mut() {
        if event.user_id.is_none() {
            if let Some(uid) = lookup.get(event.anonymous_id.as_str()) {
                event.user_id = Some(uid.to_string());
            }
        }
    }

    Ok(events)
}
