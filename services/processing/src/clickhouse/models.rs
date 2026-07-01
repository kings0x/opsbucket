use std::collections::HashMap;

use clickhouse::Row;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Row)]
pub struct ClickHouseRow {
    pub project_id: String,
    pub event_id: String,
    pub event_name: String,
    pub event_type: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub timestamp: u32,
    pub received_at: u32,
    pub original_timestamp: u32,
    pub properties: HashMap<String, String>,
    pub traits: HashMap<String, String>,
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
