CREATE TABLE IF NOT EXISTS events (
    project_id String,
    event_id String,
    event_name String,
    event_type String,
    anonymous_id String,
    user_id Nullable(String),
    timestamp DateTime,
    received_at DateTime,
    original_timestamp DateTime,
    properties Map(String, String),
    traits Map(String, String),
    user_agent String,
    locale String,
    timezone String,
    ip String,
    library_name String,
    library_version String,
    page_url String,
    page_path String,
    page_referrer String,
    page_title String,
    page_search String,
    screen_width UInt16,
    screen_height UInt16,
    screen_density Float32,
    campaign_source Nullable(String),
    campaign_medium Nullable(String),
    campaign_name Nullable(String),
    campaign_term Nullable(String),
    campaign_content Nullable(String)
) ENGINE = MergeTree()
PARTITION BY toYYYYMM(timestamp)
ORDER BY (project_id, event_name, timestamp)
TTL timestamp + INTERVAL 12 MONTH;

-- V2: Uncomment when raw event query latency requires pre-aggregation.
-- This pre-aggregates daily event counts incrementally on every insert.
--
-- CREATE MATERIALIZED VIEW daily_event_counts
-- ENGINE = SummingMergeTree()
-- ORDER BY (project_id, event_name, date)
-- AS SELECT
--     project_id,
--     event_name,
--     toDate(timestamp) AS date,
--     count()           AS event_count
-- FROM events
-- GROUP BY project_id, event_name, date;
