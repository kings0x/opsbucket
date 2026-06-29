use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackEvent {
    pub message_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub original_timestamp: String,
    pub event: String,
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyEvent {
    pub message_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub anonymous_id: String,
    pub user_id: String,
    pub original_timestamp: String,
    pub traits: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEvent {
    pub message_id: String,
    #[serde(rename = "type")]
    pub event_type: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub original_timestamp: String,
    pub name: Option<String>,
    pub properties: serde_json::Value,
}
