use serde::{Deserialize, Serialize};

// ── SDK Event Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackEvent {
    pub message_id: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub original_timestamp: String,
    pub event: String,
    pub properties: serde_json::Value,
    pub context: Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentifyEvent {
    pub message_id: String,
    pub anonymous_id: String,
    pub user_id: String,
    pub original_timestamp: String,
    pub traits: serde_json::Value,
    pub context: Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageEvent {
    pub message_id: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub original_timestamp: String,
    pub name: Option<String>,
    pub properties: serde_json::Value,
    pub context: Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnyEvent {
    #[serde(rename = "track")]
    Track(TrackEvent),
    #[serde(rename = "identify")]
    Identify(IdentifyEvent),
    #[serde(rename = "page")]
    Page(PageEvent),
}

// ── Wire Format ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchPayload {
    pub sent_at: String,
    pub batch: Vec<AnyEvent>,
}

// ── Context (matches SDK 1:1) ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub library: Library,
    pub page: Page,
    pub screen: Screen,
    pub user_agent: String,
    pub locale: String,
    pub timezone: String,
    pub campaign: Campaign,
    pub ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub url: String,
    pub path: String,
    pub referrer: String,
    pub title: String,
    pub search: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub width: u16,
    pub height: u16,
    pub density: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Campaign {
    pub source: Option<String>,
    pub medium: Option<String>,
    pub name: Option<String>,
    pub term: Option<String>,
    pub content: Option<String>,
}

// ── Server-Stamped Event (produced to Kafka) ────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawEvent {
    pub project_id: String,
    pub received_at: String,
    pub sent_at: String,
    pub ip: String,

    // Event identity
    pub message_id: String,
    pub event_type: String,
    pub anonymous_id: String,
    pub user_id: Option<String>,
    pub original_timestamp: String,

    // Context
    pub context: Context,

    // Track-specific
    pub event: Option<String>,
    pub properties: Option<serde_json::Value>,

    // Identify-specific
    pub traits: Option<serde_json::Value>,

    // Page-specific
    pub name: Option<String>,
}

impl RawEvent {
    pub fn from_any(
        event: AnyEvent,
        project_id: String,
        received_at: String,
        sent_at: String,
        ip: String,
    ) -> Self {
        match event {
            AnyEvent::Track(e) => Self {
                project_id,
                received_at,
                sent_at,
                ip,
                event_type: "track".to_string(),
                message_id: e.message_id,
                anonymous_id: e.anonymous_id,
                user_id: e.user_id,
                original_timestamp: e.original_timestamp,
                context: e.context,
                event: Some(e.event),
                properties: Some(e.properties),
                traits: None,
                name: None,
            },
            AnyEvent::Identify(e) => Self {
                project_id,
                received_at,
                sent_at,
                ip,
                event_type: "identify".to_string(),
                message_id: e.message_id,
                anonymous_id: e.anonymous_id,
                user_id: Some(e.user_id),
                original_timestamp: e.original_timestamp,
                context: e.context,
                event: None,
                properties: None,
                traits: Some(e.traits),
                name: None,
            },
            AnyEvent::Page(e) => Self {
                project_id,
                received_at,
                sent_at,
                ip,
                event_type: "page".to_string(),
                message_id: e.message_id,
                anonymous_id: e.anonymous_id,
                user_id: e.user_id,
                original_timestamp: e.original_timestamp,
                context: e.context,
                event: None,
                properties: Some(e.properties),
                traits: None,
                name: e.name,
            },
        }
    }
}
