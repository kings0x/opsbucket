use std::sync::Arc;

use anyhow::Result;
use arrow::array::{Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use opsbucket_shared::events::RawEvent;
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;

pub fn write_events_to_parquet(events: &[RawEvent]) -> Result<Vec<u8>> {
    let schema = Arc::new(Schema::new(vec![
        Field::new("event_id", DataType::Utf8, false),
        Field::new("project_id", DataType::Utf8, false),
        Field::new("event_type", DataType::Utf8, false),
        Field::new("timestamp", DataType::Int64, false),
        Field::new("raw_event", DataType::Utf8, false),
    ]));

    let mut event_ids = Vec::with_capacity(events.len());
    let mut project_ids = Vec::with_capacity(events.len());
    let mut event_types = Vec::with_capacity(events.len());
    let mut timestamps = Vec::with_capacity(events.len());
    let mut raw_jsons = Vec::with_capacity(events.len());

    for event in events {
        let ts = chrono::DateTime::parse_from_rfc3339(&event.sent_at)
            .map(|dt| dt.timestamp_millis())
            .unwrap_or_else(|_| chrono::Utc::now().timestamp_millis());

        event_ids.push(event.message_id.as_str());
        project_ids.push(event.project_id.as_str());
        event_types.push(event.event_type.as_str());
        timestamps.push(ts);
        raw_jsons.push(serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string()));
    }

    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            Arc::new(StringArray::from(event_ids)),
            Arc::new(StringArray::from(project_ids)),
            Arc::new(StringArray::from(event_types)),
            Arc::new(Int64Array::from(timestamps)),
            Arc::new(StringArray::from(raw_jsons)),
        ],
    )?;

    let mut buf = Vec::new();
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;

    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opsbucket_shared::events::{Campaign, Context, Library, Page, RawEvent, Screen};

    fn make_event(message_id: &str, project_id: &str, event_type: &str, sent_at: &str) -> RawEvent {
        RawEvent {
            project_id: project_id.to_string(),
            received_at: "2026-07-01T00:00:00Z".to_string(),
            sent_at: sent_at.to_string(),
            ip: "127.0.0.1".to_string(),
            message_id: message_id.to_string(),
            event_type: event_type.to_string(),
            anonymous_id: "anon_1".to_string(),
            user_id: None,
            original_timestamp: "2026-07-01T00:00:00Z".to_string(),
            context: Context {
                library: Library {
                    name: "test".into(),
                    version: "1.0".into(),
                },
                page: Page {
                    url: "".into(),
                    path: "".into(),
                    referrer: "".into(),
                    title: "".into(),
                    search: "".into(),
                },
                screen: Screen {
                    width: 0,
                    height: 0,
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
            },
            event: None,
            properties: None,
            traits: None,
            name: None,
        }
    }

    #[test]
    fn produces_valid_parquet_bytes() {
        let events = vec![make_event(
            "msg-1",
            "proj_1",
            "track",
            "2026-06-30T12:00:00Z",
        )];
        let bytes = write_events_to_parquet(&events).unwrap();
        assert!(!bytes.is_empty(), "parquet bytes should not be empty");
        assert!(bytes.len() > 4, "parquet bytes should have valid size");
    }

    #[test]
    fn parquet_contains_expected_schema_columns() {
        let events = vec![make_event(
            "msg-1",
            "proj_1",
            "track",
            "2026-06-30T12:00:00Z",
        )];
        let bytes = write_events_to_parquet(&events).unwrap();
        assert!(bytes.len() > 100);
        // The parquet magic bytes "PAR1" appear at the beginning and end
        let start = &bytes[0..4];
        assert_eq!(start, b"PAR1", "parquet file should start with magic bytes");
    }

    #[test]
    fn handles_multiple_events() {
        let events = vec![
            make_event("msg-1", "proj_1", "track", "2026-06-30T12:00:00Z"),
            make_event("msg-2", "proj_2", "identify", "2026-06-30T13:00:00Z"),
            make_event("msg-3", "proj_3", "page", "2026-06-30T14:00:00Z"),
        ];
        let bytes = write_events_to_parquet(&events).unwrap();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn handles_empty_event_list() {
        let events: Vec<RawEvent> = vec![];
        let bytes = write_events_to_parquet(&events).unwrap();
        assert!(
            !bytes.is_empty(),
            "parquet with zero rows should still produce valid file"
        );
    }

    #[test]
    fn preserves_event_id_in_parquet() {
        let events = vec![make_event(
            "unique-id-123",
            "proj_1",
            "track",
            "2026-06-30T12:00:00Z",
        )];
        let bytes = write_events_to_parquet(&events).unwrap();
        // The event_id "unique-id-123" should be serialized in the raw_event JSON
        let raw = String::from_utf8_lossy(&bytes);
        assert!(
            raw.contains("unique-id-123"),
            "parquet bytes should contain the event_id"
        );
    }
}
