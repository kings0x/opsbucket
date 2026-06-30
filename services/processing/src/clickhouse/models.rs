use chrono::{DateTime, Utc};
use clickhouse::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
#[serde(rename_all = "snake_case")]
pub struct ClickHouseRow {
    pub event_id: String,
    pub event_name: String,
    pub project_id: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub original_timestamp: DateTime<Utc>,
    pub properties: String,
    pub traits: String,
    pub user_agent: String,
    pub locale: String,
    pub timezone: String,
    pub ip: String,
    pub library_name: String,
    pub library_version: String,
    pub page_url: String,
    pub page_path: String,
    pub page_referrer: String,
    pub page_title: String,
    pub page_search: String,
    pub screen_width: u16,
    pub screen_height: u16,
    pub screen_density: f32,
    pub campaign_source: Option<String>,
    pub campaign_medium: Option<String>,
    pub campaign_name: Option<String>,
    pub campaign_term: Option<String>,
    pub campaign_content: Option<String>,
}
