use std::sync::Arc;

use anyhow::Result;
use arrow::array::{Int64Array, StringArray};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use opsbucket_shared::events::RawEvent;
use parquet::arrow::ArrowWriter;
use parquet::basic::{Compression, ZstdLevel};
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
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();
    let mut writer = ArrowWriter::try_new(&mut buf, schema, Some(props))?;
    writer.write(&batch)?;
    writer.close()?;

    Ok(buf)
}
