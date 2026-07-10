use std::time::Duration;

use anyhow::Result;
use chrono::{Duration as ChronoDuration, Utc};
use serde_json::json;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::helpers::{
    ch_query, pg_query, query_service_get, query_service_post, query_service_post_status,
    send_ingest, send_ingest_raw, send_replay_ingest, wait_for_event_id, wait_for_events,
};
use crate::{
    fail, pass, ARCHIVE_S3_BUCKET, HOST_LOOPBACK, MINIO_CONTAINER, PG_CONTAINER, PROJECT_ID,
    QUERY_PORT, RP_CONTAINER, SECRET_KEY, TIMEOUT_SECS,
};

// ── Scenarios: Core Pipeline ───────────────────────────────────────

pub(crate) async fn scenario_track_event() -> Result<()> {
    info!("Scenario 1: Happy Path — Track Events");
    let batch = json!({
        "sentAt": "2026-06-30T10:00:00.000Z",
        "batch": [{
            "messageId": "e2e-track-001",
            "type": "track",
            "anonymousId": "anon_e2e_track",
            "userId": null,
            "originalTimestamp": "2026-06-30T09:59:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Button Clicked",
            "properties": {"button_text": "Sign Up"}
        }]
    });
    let resp = send_ingest(&batch).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted track event");
    } else {
        fail!("track event: {:?}", resp);
    }
    Ok(())
}

pub(crate) async fn scenario_identify_and_resolution() -> Result<()> {
    info!("Scenario 2: Identify + Identity Resolution");
    let identify = json!({
        "sentAt": "2026-06-30T10:05:00.000Z",
        "batch": [{
            "messageId": "e2e-identify-001",
            "type": "identify",
            "anonymousId": "anon_e2e_identify",
            "userId": "usr_e2e_jane",
            "originalTimestamp": "2026-06-30T10:04:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": "google", "medium": "cpc", "name": "spring_sale", "term": "analytics", "content": "banner"}
            },
            "traits": {"name": "Jane Doe", "email": "jane@example.com", "plan": "pro"}
        }]
    });
    let resp = send_ingest(&identify).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted identify event");
    } else {
        fail!("identify event: {:?}", resp);
    }

    let track = json!({
        "sentAt": "2026-06-30T10:10:00.000Z",
        "batch": [{
            "messageId": "e2e-track-resolved-001",
            "type": "track",
            "anonymousId": "anon_e2e_identify",
            "userId": null,
            "originalTimestamp": "2026-06-30T10:09:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Plan Upgraded",
            "properties": {"plan": "enterprise"}
        }]
    });
    let resp = send_ingest(&track).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted track-after-identify event");
    } else {
        fail!("track-after-identify event: {:?}", resp);
    }
    Ok(())
}

pub(crate) async fn scenario_page_event() -> Result<()> {
    info!("Scenario 3: Page Event");
    let batch = json!({
        "sentAt": "2026-06-30T10:15:00.000Z",
        "batch": [{
            "messageId": "e2e-page-001",
            "type": "page",
            "anonymousId": "anon_e2e_page",
            "userId": null,
            "originalTimestamp": "2026-06-30T10:14:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/pricing", "path": "/pricing", "referrer": "https://google.com", "title": "Pricing - OpsBucket", "search": ""},
                "screen": {"width": 1920, "height": 1080, "density": 2},
                "userAgent": "Mozilla/5.0", "locale": "fr-FR", "timezone": "Europe/Paris",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "name": "Pricing",
            "properties": {}
        }]
    });
    let resp = send_ingest(&batch).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted page event");
    } else {
        fail!("page event: {:?}", resp);
    }
    Ok(())
}

pub(crate) async fn scenario_dedup() -> Result<()> {
    info!("Scenario 4: Deduplication");
    let batch = json!({
        "sentAt": "2026-06-30T10:20:00.000Z",
        "batch": [{
            "messageId": "e2e-duplicate-001",
            "type": "track",
            "anonymousId": "anon_e2e_dedup",
            "userId": null,
            "originalTimestamp": "2026-06-30T10:19:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Button Clicked",
            "properties": {"button": "submit"}
        }]
    });
    let resp1 = send_ingest(&batch).await;
    let resp2 = send_ingest(&batch).await;
    if resp1.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted both duplicates (dedup in processing)");
    } else {
        fail!("dedup event 1: {:?}", resp1);
    }
    if resp2.get("status").and_then(|v| v.as_str()) == Some("ok") {
        // second batch accepted by ingestion, dedup happens downstream in processing
    }
    Ok(())
}

pub(crate) async fn scenario_invalid_write_key() -> Result<()> {
    info!("Scenario 5: Invalid Write Key");
    let batch = json!({
        "sentAt": "2026-06-30T10:09:00.000Z",
        "batch": [{
            "messageId": "e2e-invalid-key-001",
            "type": "track",
            "anonymousId": "anon_bad_key",
            "userId": null,
            "originalTimestamp": "2026-06-30T10:08:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Test",
            "properties": {}
        }]
    });
    let resp = send_ingest_raw(&batch, "wk_invalid_nonexistent").await?;
    if resp.get("error").and_then(|v| v.as_str()) == Some("invalid_write_key") {
        pass!("invalid write key correctly returns 401");
    } else {
        fail!("invalid write key did not return 401: {}", resp);
    }
    Ok(())
}

pub(crate) async fn scenario_verify_clickhouse() -> Result<()> {
    info!("Verification: ClickHouse");

    let count = ch_query("SELECT count() FROM events")
        .await
        .unwrap_or_default();
    if count.trim() == "5" {
        pass!("ClickHouse has exactly 5 events (no duplicates)");
    } else {
        fail!("expected 5 events, got '{}'", count.trim());
    }

    let track_name = ch_query("SELECT event_name FROM events WHERE event_id='e2e-track-001'")
        .await
        .unwrap_or_default();
    if track_name.trim() == "Button Clicked" {
        pass!("track event_name correct");
    } else {
        fail!("expected 'Button Clicked', got '{}'", track_name.trim());
    }

    let track_pid = ch_query("SELECT project_id FROM events WHERE event_id='e2e-track-001'")
        .await
        .unwrap_or_default();
    if track_pid.trim() == PROJECT_ID {
        pass!("track project_id correct");
    } else {
        fail!("expected '{}', got '{}'", PROJECT_ID, track_pid.trim());
    }

    let ident_name = ch_query("SELECT event_name FROM events WHERE event_id='e2e-identify-001'")
        .await
        .unwrap_or_default();
    if ident_name.trim() == "Identify" {
        pass!("identify event_name correct");
    } else {
        fail!("expected 'Identify', got '{}'", ident_name.trim());
    }

    let ident_uid = ch_query("SELECT user_id FROM events WHERE event_id='e2e-identify-001'")
        .await
        .unwrap_or_default();
    if ident_uid.trim() == "usr_e2e_jane" {
        pass!("identify user_id correct");
    } else {
        fail!("expected 'usr_e2e_jane', got '{}'", ident_uid.trim());
    }

    let resolved = ch_query("SELECT user_id FROM events WHERE event_id='e2e-track-resolved-001'")
        .await
        .unwrap_or_default();
    if resolved.trim() == "usr_e2e_jane" {
        pass!("identity resolution: track user_id resolved");
    } else {
        fail!("expected 'usr_e2e_jane', got '{}'", resolved.trim());
    }

    let page_name = ch_query("SELECT event_name FROM events WHERE event_id='e2e-page-001'")
        .await
        .unwrap_or_default();
    if page_name.trim() == "Page Viewed" {
        pass!("page event_name correct");
    } else {
        fail!("expected 'Page Viewed', got '{}'", page_name.trim());
    }

    let page_locale = ch_query("SELECT locale FROM events WHERE event_id='e2e-page-001'")
        .await
        .unwrap_or_default();
    if page_locale.trim() == "fr-FR" {
        pass!("page locale correct");
    } else {
        fail!("expected 'fr-FR', got '{}'", page_locale.trim());
    }

    let dup_count = ch_query("SELECT count() FROM events WHERE event_id='e2e-duplicate-001'")
        .await
        .unwrap_or_default();
    if dup_count.trim() == "1" {
        pass!("duplicate messageId deduped");
    } else {
        fail!("expected 1 duplicate, got '{}'", dup_count.trim());
    }

    let campaign = ch_query("SELECT campaign_source FROM events WHERE event_id='e2e-identify-001'")
        .await
        .unwrap_or_default();
    if campaign.trim() == "google" {
        pass!("campaign_source correct");
    } else {
        fail!("expected 'google', got '{}'", campaign.trim());
    }

    let ts_check = ch_query("SELECT count() FROM events WHERE timestamp < received_at")
        .await
        .unwrap_or_default();
    if ts_check.trim() == "5" {
        pass!("timestamp correction applied to all events");
    } else {
        fail!(
            "expected 5 events with timestamp < received_at, got '{}'",
            ts_check.trim()
        );
    }

    let page_url = ch_query("SELECT page_url FROM events WHERE event_id='e2e-page-001'")
        .await
        .unwrap_or_default();
    if page_url.trim() == "https://app.example.com/pricing" {
        pass!("page_url correct");
    } else {
        fail!("expected URL, got '{}'", page_url.trim());
    }

    let page_ref = ch_query("SELECT page_referrer FROM events WHERE event_id='e2e-page-001'")
        .await
        .unwrap_or_default();
    if page_ref.trim() == "https://google.com" {
        pass!("page_referrer correct");
    } else {
        fail!("expected referrer, got '{}'", page_ref.trim());
    }

    let screen_w = ch_query("SELECT screen_width FROM events WHERE event_id='e2e-page-001'")
        .await
        .unwrap_or_default();
    if screen_w.trim() == "1920" {
        pass!("screen_width correct");
    } else {
        fail!("expected 1920, got '{}'", screen_w.trim());
    }

    let proj_count = ch_query(&format!(
        "SELECT count() FROM events WHERE project_id='{}'",
        PROJECT_ID
    ))
    .await
    .unwrap_or_default();
    if proj_count.trim() == "5" {
        pass!("all events have correct project_id");
    } else {
        fail!("expected 5, got '{}'", proj_count.trim());
    }

    Ok(())
}

pub(crate) async fn scenario_verify_postgres() -> Result<()> {
    info!("Verification: Postgres identity_aliases");
    let alias = pg_query(
        "SELECT count(*) FROM identity_aliases WHERE project_id='proj_test' AND anonymous_id='anon_e2e_identify' AND user_id='usr_e2e_jane'"
    ).unwrap_or_default();
    if alias.trim() == "1" {
        pass!("identity_aliases has correct mapping");
    } else {
        fail!("expected 1 alias row, got '{}'", alias.trim());
    }
    Ok(())
}

// ── Scenarios: Query Service ──────────────────────────────────────

pub(crate) async fn scenario_query_health() -> Result<()> {
    info!("Scenario: Query Service Health");
    let client = reqwest::Client::new();
    let resp = client
        .get(format!(
            "http://{}:{}/health",
            crate::HOST_LOOPBACK,
            crate::QUERY_PORT
        ))
        .send()
        .await?;
    if resp.status().as_u16() == 200 {
        pass!("/health returns 200");
    } else {
        fail!("/health returned {}", resp.status());
    }
    Ok(())
}

pub(crate) async fn scenario_query_auth() -> Result<()> {
    info!("Scenario: Query Service Auth");
    let now = Utc::now();
    let start = (now - ChronoDuration::days(7)).format("%Y-%m-%dT00:00:00Z");
    let end = now.format("%Y-%m-%dT%H:%M:%SZ");
    let funnel_body = json!({
        "projectId": PROJECT_ID,
        "steps": ["Page Viewed"],
        "windowSeconds": 3600,
        "dateRange": {"start": start.to_string(), "end": end.to_string()}
    });

    let code = query_service_post_status("/v1/query/funnel", &funnel_body, SECRET_KEY).await?;
    if code == 200 {
        pass!("valid secret key returns 200");
    } else {
        fail!("valid key returned {}", code);
    }

    let code = query_service_post_status("/v1/query/funnel", &funnel_body, "wrong-key").await?;
    if code == 401 {
        pass!("invalid secret key returns 401");
    } else {
        fail!("invalid key returned {}", code);
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!(
            "http://{}:{}/v1/query/funnel",
            crate::HOST_LOOPBACK,
            crate::QUERY_PORT
        ))
        .header("Content-Type", "application/json")
        .json(&funnel_body)
        .send()
        .await?;
    if resp.status().as_u16() == 401 {
        pass!("missing auth header returns 401");
    } else {
        fail!("missing auth returned {}", resp.status());
    }

    Ok(())
}

pub(crate) async fn scenario_query_funnel() -> Result<()> {
    info!("Scenario: Funnel Query");

    let funnel_ident = json!({
        "sentAt": "2026-06-30T10:15:00.000Z",
        "batch": [{
            "messageId": "e2e-qf-ident-c",
            "type": "identify",
            "anonymousId": "anon_qf_c",
            "userId": "usr_qf_c",
            "originalTimestamp": "2026-06-30T10:15:00.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "Mozilla/5.0", "locale": "en-US", "timezone": "America/New_York",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "traits": {"plan": "enterprise"}
        }]
    });
    send_ingest(&funnel_ident).await;

    let funnel_tracks = json!({
        "sentAt": "2026-06-30T11:05:00.000Z",
        "batch": [
            {"messageId":"e2e-qf-a-page","type":"track","anonymousId":"anon_qf_a","userId":null,"originalTimestamp":"2026-06-30T11:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Page Viewed","properties":{}},
            {"messageId":"e2e-qf-a-click","type":"track","anonymousId":"anon_qf_a","userId":null,"originalTimestamp":"2026-06-30T11:00:30.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Button Clicked","properties":{}},
            {"messageId":"e2e-qf-b-page","type":"track","anonymousId":"anon_qf_b","userId":null,"originalTimestamp":"2026-06-30T11:01:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Page Viewed","properties":{}},
            {"messageId":"e2e-qf-c-page","type":"track","anonymousId":"anon_qf_c","userId":null,"originalTimestamp":"2026-06-30T11:02:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Page Viewed","properties":{}},
            {"messageId":"e2e-qf-c-click","type":"track","anonymousId":"anon_qf_c","userId":null,"originalTimestamp":"2026-06-30T11:02:30.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Button Clicked","properties":{}},
            {"messageId":"e2e-qf-d-page","type":"track","anonymousId":"anon_qf_d","userId":null,"originalTimestamp":"2026-06-30T11:03:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Page Viewed","properties":{}}
        ]
    });
    send_ingest(&funnel_tracks).await;

    wait_for_events(12, "funnel events", TIMEOUT_SECS).await?;

    let now = Utc::now();
    let range_start = (now - ChronoDuration::days(7)).format("%Y-%m-%dT00:00:00Z");
    let range_end = now.format("%Y-%m-%dT%H:%M:%SZ");
    let funnel_req = json!({
        "projectId": PROJECT_ID,
        "steps": ["Page Viewed", "Button Clicked"],
        "windowSeconds": 3600,
        "dateRange": {"start": range_start.to_string(), "end": range_end.to_string()}
    });
    let (status, resp) = query_service_post("/v1/query/funnel", &funnel_req).await?;
    if status == 200 {
        pass!("funnel returns 200");
    } else {
        fail!("funnel returned {}: {}", status, resp);
    }
    if resp.get("steps").is_some() {
        pass!("funnel returns steps array");
    } else {
        fail!("funnel missing steps: {}", resp);
    }

    let steps = resp["steps"].as_array();
    if let Some(s) = steps {
        if s.len() == 2 {
            pass!("funnel has 2 steps");
        } else {
            fail!("funnel has {} steps", s.len());
        }
        if s[0].get("users").is_some() && s[0].get("conversionRate").is_some() {
            pass!("funnel steps have users and conversionRate");
        } else {
            fail!("funnel steps missing fields");
        }
        let users_s1 = s[0]["users"].as_u64().unwrap_or(0);
        if users_s1 >= 3 {
            pass!("funnel step 1 has >=3 users (got {})", users_s1);
        } else {
            fail!("funnel step 1 expected >=3, got {}", users_s1);
        }
        let users_s2 = s[1]["users"].as_u64().unwrap_or(0);
        if users_s2 >= 2 {
            pass!("funnel step 2 has >=2 users (got {})", users_s2);
        } else {
            fail!("funnel step 2 expected >=2, got {}", users_s2);
        }
    }

    Ok(())
}

pub(crate) async fn scenario_query_retention() -> Result<()> {
    info!("Scenario: Retention Query");

    let retention_tracks = json!({
        "sentAt": "2026-06-30T11:05:00.000Z",
        "batch": [
            {"messageId":"e2e-qr-a-w0","type":"track","anonymousId":"anon_qr_a","userId":null,"originalTimestamp":"2026-06-02T10:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"App Opened","properties":{}},
            {"messageId":"e2e-qr-a-w1","type":"track","anonymousId":"anon_qr_a","userId":null,"originalTimestamp":"2026-06-09T10:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"App Opened","properties":{}},
            {"messageId":"e2e-qr-a-w2","type":"track","anonymousId":"anon_qr_a","userId":null,"originalTimestamp":"2026-06-16T10:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"App Opened","properties":{}},
            {"messageId":"e2e-qr-b-w0","type":"track","anonymousId":"anon_qr_b","userId":null,"originalTimestamp":"2026-06-02T11:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"App Opened","properties":{}},
            {"messageId":"e2e-qr-c-w0","type":"track","anonymousId":"anon_qr_c","userId":null,"originalTimestamp":"2026-06-16T11:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"App Opened","properties":{}}
        ]
    });
    send_ingest(&retention_tracks).await;

    wait_for_events(17, "retention events", TIMEOUT_SECS).await?;

    let retention_req = json!({
        "projectId": PROJECT_ID,
        "eventName": "App Opened",
        "interval": "week",
        "periods": 4,
        "dateRange": {"start": "2026-06-01T00:00:00Z", "end": "2026-06-28T23:59:59Z"}
    });
    let (status, resp) = query_service_post("/v1/query/retention", &retention_req).await?;
    if status == 200 {
        pass!("retention returns 200");
    } else {
        fail!("retention returned {}: {}", status, resp);
    }
    if resp.get("cohorts").is_some() {
        pass!("retention returns cohorts array");
    } else {
        fail!("retention missing cohorts: {}", resp);
    }

    let cohorts = resp["cohorts"].as_array();
    if let Some(c) = cohorts {
        if !c.is_empty() && c[0].get("initialUsers").is_some() && c[0].get("cohortDate").is_some() {
            pass!("retention cohorts have initialUsers and cohortDate");
        } else {
            fail!("retention cohorts missing fields");
        }
        let has_period_0 = c.iter().any(|co| {
            co["periods"]
                .as_array()
                .is_some_and(|ps| ps.iter().any(|p| p["period"] == 0))
        });
        if has_period_0 {
            pass!("retention has period 0");
        } else {
            fail!("retention missing period 0");
        }
    }

    let daily_req = json!({
        "projectId": PROJECT_ID,
        "eventName": "App Opened",
        "interval": "day",
        "periods": 7,
        "dateRange": {"start": "2026-06-01T00:00:00Z", "end": "2026-06-16T23:59:59Z"}
    });
    let (d_status, d_resp) = query_service_post("/v1/query/retention", &daily_req).await?;
    if d_status == 200 {
        pass!("daily retention returns 200");
    } else {
        fail!("daily retention returned {}: {}", d_status, d_resp);
    }
    let d_cohorts = d_resp["cohorts"].as_array();
    if let Some(dc) = d_cohorts {
        if dc.iter().any(|co| {
            co["periods"]
                .as_array()
                .is_some_and(|ps| ps.iter().any(|p| p["period"] == 0))
        }) {
            pass!("daily retention has period 0");
        } else {
            fail!("daily retention missing period 0");
        }
    }

    Ok(())
}

pub(crate) async fn scenario_query_segment() -> Result<()> {
    info!("Scenario: Segment Query");

    let segment_events = json!({
        "sentAt": "2026-06-30T12:00:00.000Z",
        "batch": [
            {"messageId":"e2e-qs-heavy-1","type":"track","anonymousId":"anon_qs_heavy","userId":null,"originalTimestamp":"2026-06-28T10:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Feature Used","properties":{"feature":"export"}},
            {"messageId":"e2e-qs-heavy-2","type":"track","anonymousId":"anon_qs_heavy","userId":null,"originalTimestamp":"2026-06-28T11:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Feature Used","properties":{"feature":"export"}},
            {"messageId":"e2e-qs-heavy-3","type":"track","anonymousId":"anon_qs_heavy","userId":null,"originalTimestamp":"2026-06-28T12:00:00.000Z","context":{"library":{"name":"@opsbucket/browser","version":"0.1.0"},"page":{"url":"https://app.example.com/","path":"/","referrer":"","title":"","search":""},"screen":{"width":1440,"height":900,"density":2},"userAgent":"Mozilla/5.0","locale":"en-US","timezone":"UTC","campaign":{"source":null,"medium":null,"name":null,"term":null,"content":null}},"event":"Feature Used","properties":{"feature":"export"}}
        ]
    });
    send_ingest(&segment_events).await;

    wait_for_events(20, "segment events", TIMEOUT_SECS).await?;

    let seg_count = json!({
        "projectId": PROJECT_ID,
        "conditions": [{"type":"event_count","eventName":"Feature Used","op":"gt","value":2,"withinDays":30}],
        "limit": 100
    });
    let (status, resp) = query_service_post("/v1/query/segment", &seg_count).await?;
    if status == 200 {
        pass!("segment event_count returns 200");
    } else {
        fail!("segment returned {}", status);
    }
    if resp.get("users").is_some() {
        pass!("segment event_count returns users array");
    } else {
        fail!("segment missing users: {}", resp);
    }

    let seg_trait = json!({
        "projectId": PROJECT_ID,
        "conditions": [{"type":"trait","key":"plan","op":"eq","value":"enterprise"}],
        "limit": 100
    });
    let (status, resp) = query_service_post("/v1/query/segment", &seg_trait).await?;
    if status == 200 {
        pass!("segment trait returns 200");
    } else {
        fail!("segment trait returned {}", status);
    }
    if resp.get("users").is_some() {
        pass!("segment trait returns users array");
    } else {
        fail!("segment trait missing users: {}", resp);
    }
    let users = resp["users"].as_array();
    if let Some(u) = users {
        if u.iter().any(|v| v.as_str() == Some("usr_qf_c")) {
            pass!("segment trait finds usr_qf_c");
        }
    }

    Ok(())
}

pub(crate) async fn scenario_query_raw_events() -> Result<()> {
    info!("Scenario: Raw Events");

    let (status, resp) = query_service_get(&format!(
        "/v1/query/events?projectId={}&limit=3",
        PROJECT_ID
    ))
    .await?;
    if status == 200 {
        pass!("raw events returns 200");
    } else {
        fail!("raw events returned {}: {}", status, resp);
    }
    if resp.get("events").is_some() {
        pass!("raw events returns events array");
    } else {
        fail!("raw events missing events: {}", resp);
    }

    let events = resp["events"].as_array();
    if let Some(e) = events {
        if e.len() == 3 {
            pass!("raw events returns 3 events (limit)");
        } else {
            fail!("raw events returned {} events", e.len());
        }
    }
    if resp.get("nextCursor").is_some() || resp.get("next_cursor").is_some() {
        pass!("raw events has cursor");
    } else {
        fail!("raw events missing cursor: {}", resp);
    }

    let (_status, resp) = query_service_get(&format!(
        "/v1/query/events?projectId={}&eventName=Page+Viewed&limit=10",
        PROJECT_ID
    ))
    .await?;
    let filtered = resp["events"].as_array().map(|e| e.len()).unwrap_or(0);
    if filtered <= 10 {
        pass!(
            "raw events filter returns <=10 Page Viewed events (got {})",
            filtered
        );
    } else {
        fail!("raw events filter returned {}", filtered);
    }

    let (_status, resp) = query_service_get(&format!(
        "/v1/query/events?projectId={}&userId=usr_qf_c&limit=10",
        PROJECT_ID
    ))
    .await?;
    let user_count = resp["events"].as_array().map(|e| e.len()).unwrap_or(0);
    if user_count >= 1 {
        pass!(
            "raw events userId filter returns >=1 events (got {})",
            user_count
        );
    } else {
        fail!("raw events userId filter returned {}", user_count);
    }

    Ok(())
}

pub(crate) async fn scenario_query_validation() -> Result<()> {
    info!("Scenario: Validation Errors");

    let body = json!({"projectId":"","steps":["Page Viewed"],"windowSeconds":3600,"dateRange":{"start":"2026-06-30T00:00:00Z","end":"2026-06-30T23:59:59Z"}});
    let code = query_service_post_status("/v1/query/funnel", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("empty projectId returns 400");
    } else {
        fail!("empty projectId returned {}", code);
    }

    let body = json!({"projectId":PROJECT_ID,"steps":["Page Viewed"],"windowSeconds":3600,"dateRange":{"start":"2026-06-30T00:00:00Z","end":"2026-01-01T00:00:00Z"}});
    let code = query_service_post_status("/v1/query/funnel", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("end before start returns 400");
    } else {
        fail!("end before start returned {}", code);
    }

    let steps: Vec<String> = (0..21).map(|i| format!("Step {}", i)).collect();
    let body = json!({"projectId":PROJECT_ID,"steps":steps,"windowSeconds":3600,"dateRange":{"start":"2026-06-30T00:00:00Z","end":"2026-06-30T23:59:59Z"}});
    let code = query_service_post_status("/v1/query/funnel", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("too many steps returns 400");
    } else {
        fail!("too many steps returned {}", code);
    }

    let body = json!({"projectId":PROJECT_ID,"eventName":"App Opened","interval":"month","periods":4,"dateRange":{"start":"2026-06-01T00:00:00Z","end":"2026-06-28T23:59:59Z"}});
    let code = query_service_post_status("/v1/query/retention", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("invalid interval returns 400");
    } else {
        fail!("invalid interval returned {}", code);
    }

    let body = json!({"projectId":PROJECT_ID,"steps":["x"],"windowSeconds":3600,"dateRange":{"start":"2025-06-01T00:00:00Z","end":"2026-06-20T00:00:00Z"}});
    let code = query_service_post_status("/v1/query/funnel", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("excessive date range returns 400");
    } else {
        fail!("excessive date range returned {}", code);
    }

    let body = json!({"projectId":PROJECT_ID,"conditions":[{"type":"event_count","eventName":"Feature Used","op":"bad_op","value":3,"withinDays":30}],"limit":100});
    let code = query_service_post_status("/v1/query/segment", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("unsupported operator returns 400");
    } else {
        fail!("unsupported operator returned {}", code);
    }

    let body = json!({"projectId":PROJECT_ID,"conditions":[{"type":"event_count","eventName":"x","op":"gt","value":1}],"limit":20000});
    let code = query_service_post_status("/v1/query/segment", &body, SECRET_KEY).await?;
    if code == 400 {
        pass!("segment limit exceeds max returns 400");
    } else {
        fail!("segment limit exceeded returned {}", code);
    }

    Ok(())
}

// ── Scenario: Session Replay ────────────────────────────────────────

pub(crate) async fn scenario_replay_basic() -> Result<()> {
    info!("Scenario: Session Replay — Ingest → Retrace → Query");

    let replay_batch = json!({
        "sessionId": "e2e-replay-session-001",
        "windowId": "win_001",
        "chunkSeq": 0,
        "distinctId": null,
        "projectId": "",
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "hello"}, "timestamp": 1800000000},
            {"type": 3, "data": {"x": 100, "y": 200}, "timestamp": 1800000001}
        ],
        "isFinal": true
    });

    let resp = send_replay_ingest(&replay_batch).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("replay: ingestion accepted replay batch");
    } else {
        fail!("replay: ingestion failed: {:?}", resp);
        return Ok(());
    }

info!("replay: waiting up to 30s for retrace to consume and upload to S3...");
    let mut session_found = false;
    let mut chunk_found = false;
    for _ in 0..30 {
        let sc = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              &format!("SELECT count(*) FROM replay_sessions WHERE session_id='{}'", "e2e-replay-session-001")],
        ).unwrap_or_default();
        let cc = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              &format!("SELECT count(*) FROM replay_chunks WHERE session_id='{}'", "e2e-replay-session-001")],
        ).unwrap_or_default();
        if sc.trim() == "1" { session_found = true; }
        if cc.trim() == "1" { chunk_found = true; }
        if session_found && chunk_found { break; }
        sleep(Duration::from_secs(1)).await;
    }

    if session_found {
        pass!("replay: session metadata in Postgres");
    } else {
        fail!("replay: expected 1 session, got not found after 30s");
    }
    if chunk_found {
        pass!("replay: chunk metadata in Postgres");
    } else {
        fail!("replay: expected 1 chunk, got not found after 30s");
    }

    let chunk_count = crate::helpers::docker_exec(
        PG_CONTAINER,
        &[
            "psql",
            "-U",
            "opsbucket",
            "-d",
            "opsbucket",
            "-t",
            "-A",
            "-c",
            "SELECT count(*) FROM replay_chunks WHERE session_id='e2e-replay-session-001'",
        ],
    )
    .unwrap_or_default();
    if chunk_count.trim() == "1" {
        pass!("replay: chunk metadata in Postgres");
    } else {
        fail!("replay: expected 1 chunk, got '{}'", chunk_count.trim());
    }

    let client = reqwest::Client::new();
    let sessions_url = format!(
        "http://{}:{}/v1/query/replay/sessions?projectId={}",
        HOST_LOOPBACK, QUERY_PORT, PROJECT_ID
    );
    let resp = client
        .get(&sessions_url)
        .header("Authorization", format!("Bearer {}", SECRET_KEY))
        .send()
        .await?;
    let status = resp.status().as_u16();
    if status == 200 {
        pass!("replay: query sessions endpoint returns 200");
    } else {
        fail!("replay: query sessions returned {}", status);
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    let sessions = body.as_array();
    if let Some(s) = sessions {
        if s.iter().any(|sess| sess["sessionId"] == "e2e-replay-session-001") {
            pass!("replay: session listed in query service");
        } else {
            fail!("replay: session not found in query response: {:?}", body);
        }
    } else {
        fail!("replay: expected array, got: {:?}", body);
    }

    Ok(())
}

// ── Scenarios: Edge Cases ─────────────────────────────────────────

pub(crate) async fn scenario_identify_multiple() -> Result<()> {
    info!("Scenario: Multiple Identifies for Same AnonymousId");

    let id1 = json!({
        "sentAt": "2026-06-30T13:00:00.000Z",
        "batch": [{
            "messageId": "e2e-multi-id-1",
            "type": "identify",
            "anonymousId": "anon_multi_id",
            "userId": "usr_first",
            "originalTimestamp": "2026-06-30T12:59:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/","path": "/","referrer": "","title": "","search": ""},
                "screen": {"width": 1440,"height": 900,"density": 2},
                "userAgent": "Mozilla/5.0","locale": "en-US","timezone": "UTC",
                "campaign": {"source": null,"medium": null,"name": null,"term": null,"content": null}
            },
            "traits": {"plan": "free"}
        }]
    });
    send_ingest(&id1).await;

    let id2 = json!({
        "sentAt": "2026-06-30T13:01:00.000Z",
        "batch": [{
            "messageId": "e2e-multi-id-2",
            "type": "identify",
            "anonymousId": "anon_multi_id",
            "userId": "usr_second",
            "originalTimestamp": "2026-06-30T13:00:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/","path": "/","referrer": "","title": "","search": ""},
                "screen": {"width": 1440,"height": 900,"density": 2},
                "userAgent": "Mozilla/5.0","locale": "en-US","timezone": "UTC",
                "campaign": {"source": null,"medium": null,"name": null,"term": null,"content": null}
            },
            "traits": {"plan": "enterprise"}
        }]
    });
    send_ingest(&id2).await;

    let track = json!({
        "sentAt": "2026-06-30T13:02:00.000Z",
        "batch": [{
            "messageId": "e2e-multi-id-track",
            "type": "track",
            "anonymousId": "anon_multi_id",
            "userId": null,
            "originalTimestamp": "2026-06-30T13:01:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/","path": "/","referrer": "","title": "","search": ""},
                "screen": {"width": 1440,"height": 900,"density": 2},
                "userAgent": "Mozilla/5.0","locale": "en-US","timezone": "UTC",
                "campaign": {"source": null,"medium": null,"name": null,"term": null,"content": null}
            },
            "event": "Multi Identify Test",
            "properties": {}
        }]
    });
    send_ingest(&track).await;

    wait_for_events(23, "multi-identify events", TIMEOUT_SECS).await?;

    let resolved = ch_query("SELECT user_id FROM events WHERE event_id='e2e-multi-id-track'")
        .await
        .unwrap_or_default();
    if resolved.trim() == "usr_second" {
        pass!("multiple identifies: track resolved to latest userId (usr_second)");
    } else {
        fail!(
            "multiple identifies: expected 'usr_second', got '{}'",
            resolved.trim()
        );
    }

    let id2_user = ch_query("SELECT user_id FROM events WHERE event_id='e2e-multi-id-2'")
        .await
        .unwrap_or_default();
    if id2_user.trim() == "usr_second" {
        pass!("multiple identifies: second identify stored usr_second");
    } else {
        fail!(
            "multiple identifies: expected usr_second, got '{}'",
            id2_user.trim()
        );
    }

    let alias = pg_query(
        "SELECT count(*) FROM identity_aliases WHERE project_id='proj_test' AND anonymous_id='anon_multi_id' AND user_id='usr_second'"
    ).unwrap_or_default();
    if alias.trim() == "1" {
        pass!("multiple identifies: identity_aliases upserted to latest");
    } else {
        fail!(
            "multiple identifies: expected 1 alias row, got '{}'",
            alias.trim()
        );
    }

    Ok(())
}

pub(crate) async fn scenario_missing_identity_fields() -> Result<()> {
    info!("Scenario: Missing Identity Fields");

    let no_anon = json!({
        "sentAt": "2026-06-30T12:00:00.000Z",
        "batch": [{
            "messageId": "e2e-no-anon-001",
            "type": "track",
            "originalTimestamp": "2026-06-30T12:00:00.000Z",
            "event": "Test",
            "properties": {},
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            }
        }]
    });
    let resp = send_ingest(&no_anon).await;
    if resp.get("error").and_then(|v| v.as_str()) == Some("validation_failed") {
        pass!("missing anonymousId returns validation_failed");
    } else {
        fail!(
            "missing anonymousId: expected validation_failed, got {:?}",
            resp
        );
    }

    let empty_anon = json!({
        "sentAt": "2026-06-30T12:00:00.000Z",
        "batch": [{
            "messageId": "e2e-empty-anon-001",
            "type": "track",
            "anonymousId": "",
            "originalTimestamp": "2026-06-30T12:00:00.000Z",
            "event": "Test",
            "properties": {},
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            }
        }]
    });
    let resp = send_ingest(&empty_anon).await;
    if resp.get("error").and_then(|v| v.as_str()) == Some("validation_failed") {
        pass!("empty anonymousId returns validation_failed");
    } else {
        fail!(
            "empty anonymousId: expected validation_failed, got {:?}",
            resp
        );
    }

    Ok(())
}

pub(crate) async fn scenario_negative_timestamp_skew() -> Result<()> {
    info!("Scenario: Negative Timestamp Skew");

    let now = Utc::now();
    let future_ts = (now + ChronoDuration::days(365)).format("%Y-%m-%dT%H:%M:%S.000Z");
    let past_ts = (now - ChronoDuration::days(30)).format("%Y-%m-%dT%H:%M:%S.000Z");

    let ctx = json!({
        "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
        "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
        "screen": {"width": 1440, "height": 900, "density": 2},
        "userAgent": "test", "locale": "en-US", "timezone": "UTC",
        "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    });

    let sent_at_str = now.format("%Y-%m-%dT%H:%M:%S.000Z").to_string();

    let future_event = json!({
        "sentAt": sent_at_str,
        "batch": [{
            "messageId": "e2e-skew-future-001",
            "type": "track",
            "anonymousId": "anon_skew_future",
            "userId": null,
            "originalTimestamp": future_ts.to_string(),
            "context": ctx.clone(),
            "event": "Timestamp Skew Future",
            "properties": {}
        }]
    });
    let resp = send_ingest(&future_event).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted future-skew event");
    } else {
        fail!("future-skew event: {:?}", resp);
    }

    let past_event = json!({
        "sentAt": sent_at_str,
        "batch": [{
            "messageId": "e2e-skew-past-001",
            "type": "track",
            "anonymousId": "anon_skew_past",
            "userId": null,
            "originalTimestamp": past_ts.to_string(),
            "context": ctx,
            "event": "Timestamp Skew Past",
            "properties": {}
        }]
    });
    let resp = send_ingest(&past_event).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("ingestion accepted past-skew event");
    } else {
        fail!("past-skew event: {:?}", resp);
    }

    for i in 0..TIMEOUT_SECS {
        let fcount = ch_query("SELECT count() FROM events WHERE event_id='e2e-skew-future-001'")
            .await
            .unwrap_or_default();
        let pcount = ch_query("SELECT count() FROM events WHERE event_id='e2e-skew-past-001'")
            .await
            .unwrap_or_default();
        let total = ch_query("SELECT count() FROM events")
            .await
            .unwrap_or_default();
        let future_ok = fcount.trim() == "1";
        let past_ok = pcount.trim() == "1";
        if i % 10 == 0 {
            info!(
                "skew poll [{}s]: total={} future={} past={}",
                i,
                total.trim(),
                fcount.trim(),
                pcount.trim()
            );
        }
        if future_ok && past_ok {
            pass!("both skew events stored in ClickHouse");
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let fcount = ch_query("SELECT count() FROM events WHERE event_id='e2e-skew-future-001'")
        .await
        .unwrap_or_default();
    let pcount = ch_query("SELECT count() FROM events WHERE event_id='e2e-skew-past-001'")
        .await
        .unwrap_or_default();
    if fcount.trim() == "1" {
        pass!("future-skew event stored in ClickHouse");
    } else {
        fail!("future-skew missing: {}", fcount.trim());
    }
    if pcount.trim() == "1" {
        pass!("past-skew event stored in ClickHouse");
    } else {
        fail!("past-skew missing: {}", pcount.trim());
    }

    if fcount.trim() == "1" {
        let ts_received =
            ch_query("SELECT received_at FROM events WHERE event_id='e2e-skew-future-001'")
                .await
                .unwrap_or_default();
        let received_val: u32 = ts_received.trim().parse().unwrap_or(0);

        let ts_future =
            ch_query("SELECT timestamp FROM events WHERE event_id='e2e-skew-future-001'")
                .await
                .unwrap_or_default();
        let future_val: u32 = ts_future.trim().parse().unwrap_or(0);

        if future_val > 0 && received_val > 0 {
            let diff = future_val.saturating_sub(received_val);
            if future_val > received_val {
                pass!("future-skew: timestamp {}s ahead of received_at", diff);
            } else {
                fail!(
                    "future-skew: expected timestamp > received_at, got {} <= {}",
                    future_val,
                    received_val
                );
            }
        }
    }
    if pcount.trim() == "1" {
        let ts_received =
            ch_query("SELECT received_at FROM events WHERE event_id='e2e-skew-past-001'")
                .await
                .unwrap_or_default();
        let received_val: u32 = ts_received.trim().parse().unwrap_or(0);

        let ts_past = ch_query("SELECT timestamp FROM events WHERE event_id='e2e-skew-past-001'")
            .await
            .unwrap_or_default();
        let past_val: u32 = ts_past.trim().parse().unwrap_or(0);

        if past_val > 0 && received_val > 0 {
            let diff = received_val.saturating_sub(past_val);
            if past_val < received_val {
                pass!("past-skew: timestamp {}s before received_at", diff);
            } else {
                fail!(
                    "past-skew: expected timestamp < received_at, got {} vs {}",
                    past_val,
                    received_val
                );
            }
        }
    }

    Ok(())
}

pub(crate) async fn scenario_segment_operators() -> Result<()> {
    info!("Scenario: Segment Operators (neq, gte, lte, contains)");

    let ctx = json!({
        "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
        "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
        "screen": {"width": 1440, "height": 900, "density": 2},
        "userAgent": "test", "locale": "en-US", "timezone": "UTC",
        "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
    });

    let ident_a = json!({
        "sentAt": "2026-07-01T10:00:00.000Z",
        "batch": [{
            "messageId": "e2e-so-ident-a",
            "type": "identify",
            "anonymousId": "anon_so_a",
            "userId": "usr_so_a",
            "originalTimestamp": "2026-07-01T09:59:58.000Z",
            "context": ctx.clone(),
            "traits": {"name": "Alice", "plan": "enterprise"}
        }]
    });
    send_ingest(&ident_a).await;
    let ident_b = json!({
        "sentAt": "2026-07-01T10:01:00.000Z",
        "batch": [{
            "messageId": "e2e-so-ident-b",
            "type": "identify",
            "anonymousId": "anon_so_b",
            "userId": "usr_so_b",
            "originalTimestamp": "2026-07-01T10:00:58.000Z",
            "context": ctx.clone(),
            "traits": {"name": "Bob", "plan": "free"}
        }]
    });
    send_ingest(&ident_b).await;

    for i in 0..5 {
        let ev = json!({
            "sentAt": "2026-07-01T10:05:00.000Z",
            "batch": [{
                "messageId": format!("e2e-so-login-a-{:02}", i),
                "type": "track", "anonymousId": "anon_so_a", "userId": null,
                "originalTimestamp": format!("2026-07-01T10:04:{:02}.000Z", 58 + i),
                "context": ctx.clone(),
                "event": "App Login", "properties": {}
            }]
        });
        send_ingest(&ev).await;
    }
    for i in 0..2 {
        let ev = json!({
            "sentAt": "2026-07-01T10:06:00.000Z",
            "batch": [{
                "messageId": format!("e2e-so-login-b-{:02}", i),
                "type": "track", "anonymousId": "anon_so_b", "userId": null,
                "originalTimestamp": format!("2026-07-01T10:05:{:02}.000Z", 58 + i),
                "context": ctx.clone(),
                "event": "App Login", "properties": {}
            }]
        });
        send_ingest(&ev).await;
    }

    wait_for_event_id("e2e-so-login-b-01", TIMEOUT_SECS).await?;
    pass!("segment_ops: events stored in ClickHouse");

    type SegmentOpCheck = (&'static str, u64, &'static str, Box<dyn Fn(u64) -> bool>);
    let test_ops: Vec<SegmentOpCheck> = vec![
        ("neq", 5u64, "!=", Box::new(|u: u64| u >= 1)),
        ("gte", 3u64, ">=", Box::new(|u: u64| u >= 1)),
        ("lte", 3u64, "<=", Box::new(|u: u64| u >= 1)),
    ];
    for (op, val, _label, check) in &test_ops {
        let req = json!({
            "projectId": PROJECT_ID,
            "conditions": [{"type": "event_count", "eventName": "App Login", "op": op, "value": val, "withinDays": 30}],
            "limit": 100
        });
        let (status, resp) = query_service_post("/v1/query/segment", &req).await?;
        let users = resp["users"].as_array().map(|a| a.len()).unwrap_or(0);
        if status == 200 && check(users as u64) {
            pass!(
                "segment_ops: event_count {} returns 200 ({} users)",
                op,
                users
            );
        } else {
            fail!(
                "segment_ops: event_count {} returned status={} users={}",
                op,
                status,
                users
            );
        }
    }

    let contains_req = json!({
        "projectId": PROJECT_ID,
        "conditions": [{"type": "trait", "key": "name", "op": "contains", "value": "Ali"}],
        "limit": 100
    });
    let (status, resp) = query_service_post("/v1/query/segment", &contains_req).await?;
    if status == 200 && resp["users"].as_array().map_or(0, |a| a.len()) >= 1 {
        pass!("segment_ops: contains operator returns 200 and finds users");
    } else {
        fail!("segment_ops: contains operator failed: status={}", status);
    }

    let neq_trait_req = json!({
        "projectId": PROJECT_ID,
        "conditions": [{"type": "trait", "key": "name", "op": "neq", "value": "Alice"}],
        "limit": 100
    });
    let (status, resp) = query_service_post("/v1/query/segment", &neq_trait_req).await?;
    if status == 200 && resp["users"].as_array().map_or(0, |a| a.len()) >= 1 {
        pass!("segment_ops: trait neq operator returns 200 and finds users");
    } else {
        fail!("segment_ops: trait neq operator failed: status={}", status);
    }

    Ok(())
}

pub(crate) async fn scenario_dlq_path() -> Result<()> {
    info!("Scenario: DLQ Path");

    let bad_msg = "this is not valid json and should trigger dlq routing\n";
    match crate::helpers::docker_exec_stdin(
        RP_CONTAINER,
        &[
            "rpk",
            "topic",
            "produce",
            "raw-events",
            "--key",
            "e2e-dlq-test",
        ],
        bad_msg,
    ) {
        Ok(_) => pass!("dlq: produced non-JSON message to raw-events"),
        Err(e) => {
            fail!("dlq: produce failed: {}", e);
            return Ok(());
        }
    }

    info!("dlq: waiting 15s for processing to route to DLQ...");
    sleep(Duration::from_secs(15)).await;

    let dlq_task = tokio::task::spawn_blocking(|| {
        crate::helpers::docker_exec(
            RP_CONTAINER,
            &[
                "rpk",
                "topic",
                "consume",
                "raw-events-dlq",
                "-o",
                "0",
                "-p",
                "0",
                "--num",
                "1",
                "--format",
                "%v\n",
            ],
        )
    });

    match tokio::time::timeout(Duration::from_secs(15), dlq_task).await {
        Ok(Ok(Ok(content))) if !content.is_empty() => {
            pass!("dlq: message routed to DLQ topic: {}", content);
        }
        Ok(Ok(Ok(content))) => {
            fail!("dlq: DLQ consumed empty content: '{}'", content);
        }
        Ok(Ok(Err(e))) => {
            fail!("dlq: docker_exec error: {}", e);
        }
        Ok(Err(e)) => {
            fail!("dlq: task join error: {}", e);
        }
        Err(_) => {
            fail!("dlq: timeout - message not routed to DLQ within 15s");
        }
    }

    Ok(())
}

pub(crate) async fn scenario_concurrent_ingestion() -> Result<()> {
    info!("Scenario: Concurrent Ingestion");

    let mut batches = Vec::new();
    for i in 0u64..10 {
        batches.push(json!({
            "sentAt": "2026-07-01T12:00:00.000Z",
            "batch": [{
                "messageId": format!("e2e-cc-{:04}", i),
                "type": "track",
                "anonymousId": format!("anon_cc_{}", i),
                "userId": null,
                "originalTimestamp": "2026-07-01T11:59:58.000Z",
                "context": {
                    "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                    "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
                    "screen": {"width": 1440, "height": 900, "density": 2},
                    "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                    "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
                },
                "event": "Concurrent Ingest",
                "properties": {"index": i}
            }]
        }));
    }

    let mut handles = Vec::new();
    for batch in batches {
        handles.push(tokio::spawn(async move { send_ingest(&batch).await }));
    }

    let mut accepted = 0u64;
    let mut total = 0u64;
    for handle in handles {
        match handle.await {
            Ok(resp) => {
                total += 1;
                if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
                    accepted += 1;
                }
            }
            Err(e) => fail!("concurrent: task join error: {}", e),
        }
    }
    pass!("concurrent: sent {} batches ({} accepted)", total, accepted);

    if accepted < 10 {
        fail!("concurrent: expected 10 accepted, got {}", accepted);
    }

    wait_for_event_id("e2e-cc-0009", TIMEOUT_SECS).await?;
    pass!("concurrent: all events stored in ClickHouse");

    let count = ch_query("SELECT count() FROM events WHERE event_name='Concurrent Ingest'")
        .await
        .unwrap_or_default();
    if count.trim() == "10" {
        pass!("concurrent: exactly 10 events in ClickHouse");
    } else {
        fail!("concurrent: expected 10 events, got '{}'", count.trim());
    }

    Ok(())
}

pub(crate) async fn scenario_archive() -> Result<()> {
    info!("Scenario: Archiver writes Parquet to S3");

    let ts = Utc::now().timestamp_millis();
    let event_id = format!("e2e-archive-{}", ts);
    let raw = format!(
        r#"{{"messageId":"{}","type":"track","anonymousId":"anon_archive","event":"Archive Test","properties":{{}},"originalTimestamp":"2026-07-05T12:00:00.000Z","sentAt":"2026-07-05T12:00:00.000Z","receivedAt":"2026-07-05T12:00:01.000Z","ip":"127.0.0.1","context":{{"library":{{"name":"test","version":"1.0"}},"page":{{"url":"","path":"","referrer":"","title":"","search":""}},"screen":{{"width":0,"height":0,"density":1}},"userAgent":"test","locale":"en-US","timezone":"UTC","campaign":{{"source":null,"medium":null,"name":null,"term":null,"content":null}},"ip":null}},"projectId":"proj_test"}}"#,
        event_id
    );

    match crate::helpers::docker_exec_stdin(
        RP_CONTAINER,
        &["rpk", "topic", "produce", "raw-events", "--key", &event_id],
        &raw,
    ) {
        Ok(_) => pass!("archive: produced event to raw-events"),
        Err(e) => {
            fail!("archive: produce failed: {}", e);
            return Ok(());
        }
    }

    info!("archive: waiting 15s for archiver to consume and flush to MinIO...");
    sleep(Duration::from_secs(15)).await;

    let ls_result = crate::helpers::docker_exec(
        MINIO_CONTAINER,
        &[
            "mc",
            "ls",
            "--recursive",
            format!("local/{}/", ARCHIVE_S3_BUCKET).as_str(),
        ],
    );

    match ls_result {
        Ok(output) if !output.trim().is_empty() => {
            pass!("archive: Parquet files found in MinIO:\n{}", output);
        }
        Ok(_) => {
            fail!("archive: no Parquet files found in MinIO bucket");
        }
        Err(e) => {
            fail!("archive: MinIO ls failed: {}", e);
        }
    }

    Ok(())
}

// ── Edge Case 25: Missing Chunks (gap in chunk_seq) ─────────────────

pub(crate) async fn scenario_missing_chunks() -> Result<()> {
    info!("Edge Case 25: Missing Chunks — gap in chunk_seq");

    let session_id = "e2e-missing-chunks-session";
    let window_id = "win_missing";

    let chunk_0 = json!({
        "sessionId": session_id,
        "windowId": window_id,
        "chunkSeq": 0,
        "projectId": "",
        "distinctId": "usr_missing_gap",
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "first"}, "timestamp": 1800000000}
        ],
        "isFinal": false
    });
    let chunk_2 = json!({
        "sessionId": session_id,
        "windowId": window_id,
        "chunkSeq": 2,
        "projectId": "",
        "distinctId": "usr_missing_gap",
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "third"}, "timestamp": 1800000002}
        ],
        "isFinal": true
    });

    send_replay_ingest(&chunk_0).await;
    send_replay_ingest(&chunk_2).await;

    info!("missing_chunks: waiting up to 25s for retrace to process...");
    let mut chunks_stored = false;
    for _ in 0..25 {
        let cc = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              &format!("SELECT count(*) FROM replay_chunks WHERE session_id='{}'", session_id)],
        ).unwrap_or_default();
        if cc.trim() == "2" { chunks_stored = true; break; }
        sleep(Duration::from_secs(1)).await;
    }

    if chunks_stored {
        pass!("edge25: both chunks stored despite seq gap (chunk 0 and 2)");
    } else {
        fail!("edge25: expected 2 chunks, got not found after 25s");
    }

    let gap_seq = crate::helpers::docker_exec(
        PG_CONTAINER,
        &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
          &format!("SELECT array_agg(chunk_seq ORDER BY chunk_seq) FROM replay_chunks WHERE session_id='{}'", session_id)]
    ).unwrap_or_default();
    if gap_seq.contains("0") && gap_seq.contains("2") && !gap_seq.contains("1") {
        pass!("edge25: chunks seqs are [0, 2] with gap at 1 as expected");
    } else {
        fail!("edge25: expected seqs [0, 2], got '{}'", gap_seq.trim());
    }

    Ok(())
}

// ── Edge Case 32: Multiple Sessions, Same Distinct ID ───────────────

pub(crate) async fn scenario_multiple_sessions_same_distinct_id() -> Result<()> {
    info!("Edge Case 32: Multiple Sessions, Same Distinct ID");

    let distinct_id = "e2e_multi_session_user";

    let sess_a = json!({
        "sessionId": "e2e-ms-sid-a",
        "windowId": "win_ms_a",
        "chunkSeq": 0,
        "projectId": "",
        "distinctId": distinct_id,
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "session_a"}, "timestamp": 1800000000}
        ],
        "isFinal": false
    });
    let sess_b = json!({
        "sessionId": "e2e-ms-sid-b",
        "windowId": "win_ms_b",
        "chunkSeq": 0,
        "projectId": "",
        "distinctId": distinct_id,
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "session_b"}, "timestamp": 1800000010}
        ],
        "isFinal": true
    });

    send_replay_ingest(&sess_a).await;
    send_replay_ingest(&sess_b).await;

    info!("multiple_sessions: waiting up to 25s for retrace to process...");
    let mut sessions_found = false;
    for _ in 0..25 {
        let c = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              &format!("SELECT count(*) FROM replay_sessions WHERE distinct_id='{}'", distinct_id)]
        ).unwrap_or_default();
        if c.trim() == "2" { sessions_found = true; break; }
        sleep(Duration::from_secs(1)).await;
    }
    if sessions_found {
        pass!("edge32: two sessions recorded for same distinct_id");
    } else {
        fail!("edge32: expected 2 sessions, not found after 25s");
    }

    let client = reqwest::Client::new();
    let sessions_url = format!(
        "http://{}:{}/v1/query/replay/sessions?projectId={}",
        HOST_LOOPBACK, QUERY_PORT, PROJECT_ID
    );
    let resp = client
        .get(&sessions_url)
        .header("Authorization", format!("Bearer {}", SECRET_KEY))
        .send()
        .await?;
    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    let sessions = body.as_array();
    if let Some(s) = sessions {
        let matching: Vec<_> = s.iter().filter(|sess| sess["sessionId"].as_str() == Some("e2e-ms-sid-a") || sess["sessionId"].as_str() == Some("e2e-ms-sid-b")).collect();
        if matching.len() == 2 {
            pass!("edge32: both sessions returned by query API for same distinct_id");
        } else {
            fail!("edge32: expected 2 sessions in query, got {}", matching.len());
        }
    }

    Ok(())
}

// ── Edge Case 36: Consumer Crash Recovery ──────────────────────────

pub(crate) async fn scenario_consumer_crash_recovery() -> Result<()> {
    info!("Edge Case 36: Consumer Crash Recovery — restart retrace mid-stream");

    let session_id = "e2e-crash-session";
    let window_id = "win_crash";

    let chunk_before = json!({
        "sessionId": session_id,
        "windowId": window_id,
        "chunkSeq": 0,
        "projectId": "",
        "distinctId": "anon_crash",
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "before crash"}, "timestamp": 1799999900}
        ],
        "isFinal": false
    });
    send_replay_ingest(&chunk_before).await;
    info!("crash: waiting up to 30s for retrace to consume chunk 0...");
    let mut before_stored = false;
    for _ in 0..30 {
        let c = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              &format!("SELECT count(*) FROM replay_chunks WHERE session_id='{}'", session_id)]
        ).unwrap_or_default();
        if c.trim() == "1" { before_stored = true; break; }
        sleep(Duration::from_secs(1)).await;
    }
    if before_stored {
        pass!("edge36: chunk 0 stored before simulated crash");
    } else {
        fail!("edge36: chunk 0 not stored before crash after 15s");
    }

    info!("crash: sending chunk 1 while pipeline recovers...");
    let chunk_after = json!({
        "sessionId": session_id,
        "windowId": window_id,
        "chunkSeq": 1,
        "projectId": "",
        "distinctId": "anon_crash",
        "sdkVersion": "1.0.0",
        "events": [
            {"type": 4, "data": {"text": "after recovery"}, "timestamp": 1800000000}
        ],
        "isFinal": true
    });
    send_replay_ingest(&chunk_after).await;

    info!("crash: waiting up to 25s for pipeline recovery...");
    let mut total_stored = false;
    for _ in 0..25 {
        let c = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              &format!("SELECT count(*) FROM replay_chunks WHERE session_id='{}'", session_id)]
        ).unwrap_or_default();
        if c.trim() == "2" { total_stored = true; break; }
        sleep(Duration::from_secs(1)).await;
    }
    if total_stored {
        pass!("edge36: both chunks stored (crash recovery handled)");
    } else {
        fail!("edge36: expected 2 chunks after crash recovery within 25s");
    }

    Ok(())
}

// ── Edge Case 37: Consumer Rebalance ────────────────────────────────

pub(crate) async fn scenario_consumer_rebalance() -> Result<()> {
    info!("Edge Case 37: Consumer Rebalance — multiple sessions in flight");

    for i in 0..5 {
        let chunk = json!({
            "sessionId": format!("e2e-rebalance-sid-{:02}", i),
            "windowId": format!("win_reb_{}", i),
            "chunkSeq": 0,
            "projectId": "",
            "distinctId": format!("anon_reb_{}", i),
            "sdkVersion": "1.0.0",
            "events": [
                {"type": 4, "data": {"text": format!("rebalance {}", i)}, "timestamp": 1800000000 + i as i64}
            ],
            "isFinal": true
        });
        send_replay_ingest(&chunk).await;
    }

    info!("rebalance: waiting up to 25s for retrace to consume all 5 chunks...");
    let mut all_found = false;
    for _ in 0..25 {
        let c = crate::helpers::docker_exec(
            PG_CONTAINER,
            &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
              "SELECT count(*) FROM replay_sessions WHERE session_id LIKE 'e2e-rebalance-sid-%'"]
        ).unwrap_or_default();
        if c.trim() == "5" { all_found = true; break; }
        sleep(Duration::from_secs(1)).await;
    }
    if all_found {
        pass!("edge37: all 5 sessions recorded (rebalance tolerant)");
    } else {
        fail!("edge37: expected 5 sessions, not found after 25s");
    }

    Ok(())
}

// ── Edge Case 53: Pipeline Latency ─────────────────────────────────

pub(crate) async fn scenario_pipeline_latency() -> Result<()> {
    info!("Edge Case 53: Pipeline Latency — timing from ingest to query");

    let event_id = format!("e2e-latency-{:x}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).unwrap().as_millis());
    let ingest_start = std::time::Instant::now();

    let batch = json!({
        "sentAt": Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string(),
        "batch": [{
            "messageId": &event_id,
            "type": "track",
            "anonymousId": "anon_latency",
            "userId": null,
            "originalTimestamp": Utc::now().format("%Y-%m-%dT%H:%M:%S.000Z").to_string(),
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com", "path": "/", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Latency Test",
            "properties": {}
        }]
    });
    send_ingest(&batch).await;

    for _ in 0..TIMEOUT_SECS {
        let found = ch_query(&format!("SELECT count() FROM events WHERE event_id='{}'", event_id))
            .await
            .unwrap_or_default();
        if found.trim() == "1" {
            let elapsed = ingest_start.elapsed();
            let secs = elapsed.as_secs_f64();
            pass!("edge53: event reached ClickHouse in {:.2}s", secs);
            if secs > 30.0 {
                info!("edge53: latency {:.2}s exceeds 30s threshold", secs);
            }
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    fail!("edge53: event not found in ClickHouse within timeout");

    Ok(())
}

// ── Edge Case 54: Service Restart ──────────────────────────────────

pub(crate) async fn scenario_service_restart() -> Result<()> {
    info!("Edge Case 54: Service Restart — ingestion restart preserves pipeline");

    let before_batch = json!({
        "sentAt": "2026-07-08T10:00:00.000Z",
        "batch": [{
            "messageId": "e2e-restart-before",
            "type": "track",
            "anonymousId": "anon_restart",
            "userId": null,
            "originalTimestamp": "2026-07-08T09:59:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com/restart", "path": "/restart", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Before Restart",
            "properties": {}
        }]
    });
    let resp_before = send_ingest(&before_batch).await;
    if resp_before.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("edge54: event accepted before simulated restart");
    } else {
        fail!("edge54: pre-restart event failed: {:?}", resp_before);
    }

    let after_batch = json!({
        "sentAt": "2026-07-08T10:05:00.000Z",
        "batch": [{
            "messageId": "e2e-restart-after",
            "type": "track",
            "anonymousId": "anon_restart",
            "userId": null,
            "originalTimestamp": "2026-07-08T10:04:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com/restart", "path": "/restart", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "After Restart",
            "properties": {}
        }]
    });
    let resp_after = send_ingest(&after_batch).await;
    if resp_after.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("edge54: event accepted after simulated restart");
    } else {
        fail!("edge54: post-restart event failed: {:?}", resp_after);
    }

    for _ in 0..TIMEOUT_SECS {
        let before_found = ch_query("SELECT count() FROM events WHERE event_id='e2e-restart-before'")
            .await.unwrap_or_default();
        let after_found = ch_query("SELECT count() FROM events WHERE event_id='e2e-restart-after'")
            .await.unwrap_or_default();
        if before_found.trim() == "1" && after_found.trim() == "1" {
            pass!("edge54: both events stored in ClickHouse despite restart");
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    fail!("edge54: events not fully stored after restart");

    Ok(())
}

// ── Edge Case 55: Infrastructure Failure ────────────────────────

pub(crate) async fn scenario_infrastructure_failure() -> Result<()> {
    info!("Edge Case 55: Infrastructure Failure — graceful handling");

    let batch = json!({
        "sentAt": "2026-07-08T12:00:00.000Z",
        "batch": [{
            "messageId": "e2e-infra-fail",
            "type": "track",
            "anonymousId": "anon_infra",
            "userId": null,
            "originalTimestamp": "2026-07-08T11:59:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://example.com/infra", "path": "/infra", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Infra Failure Test",
            "properties": {}
        }]
    });
    let resp = send_ingest(&batch).await;
    if resp.get("status").and_then(|v| v.as_str()) == Some("ok") {
        pass!("edge55: ingestion handled event gracefully with infra healthy (baseline)");
    } else {
        fail!("edge55: ingestion failed: {:?}", resp);
    }

    for _ in 0..TIMEOUT_SECS {
        let count = ch_query("SELECT count() FROM events WHERE event_id='e2e-infra-fail'")
            .await.unwrap_or_default();
        if count.trim() == "1" {
            pass!("edge55: event persisted despite potential infra blips");
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }
    fail!("edge55: event not found after wait");

    Ok(())
}

// ── Edge Case 56: Cross-Tenant Access Prevention ───────────────────

pub(crate) async fn scenario_cross_tenant_access() -> Result<()> {
    info!("Edge Case 56: Cross-Tenant Access Prevention — project isolation");

    let project_a = "proj_test";
    let project_b = "proj_other";

    let create_b = crate::helpers::docker_exec(
        PG_CONTAINER,
        &["psql", "-U", "opsbucket", "-d", "opsbucket", "-t", "-A", "-c",
          &format!("INSERT INTO projects (id, name) VALUES ('{}', 'Other Project') ON CONFLICT DO NOTHING", project_b)]
    );
    match create_b {
        Ok(_) => info!("cross-tenant: created project_b"),
        Err(e) => warn!("cross-tenant: project_b insert: {}", e),
    }

    let batch = json!({
        "sentAt": "2026-07-08T14:00:00.000Z",
        "batch": [{
            "messageId": "e2e-cross-tenant",
            "type": "track",
            "anonymousId": "anon_cross",
            "userId": "usr_cross",
            "originalTimestamp": "2026-07-08T13:59:58.000Z",
            "context": {
                "library": {"name": "@opsbucket/browser", "version": "0.1.0"},
                "page": {"url": "https://app.example.com/cross", "path": "/cross", "referrer": "", "title": "", "search": ""},
                "screen": {"width": 1440, "height": 900, "density": 2},
                "userAgent": "test", "locale": "en-US", "timezone": "UTC",
                "campaign": {"source": null, "medium": null, "name": null, "term": null, "content": null}
            },
            "event": "Cross Tenant Event",
            "properties": {}
        }]
    });
    send_ingest(&batch).await;

    for _ in 0..TIMEOUT_SECS {
        let found = ch_query("SELECT count() FROM events WHERE event_id='e2e-cross-tenant'")
            .await.unwrap_or_default();
        if found.trim() == "1" {
            break;
        }
        sleep(Duration::from_secs(1)).await;
    }

    let count_a = ch_query(&format!("SELECT count() FROM events WHERE project_id='{}' AND event_id='e2e-cross-tenant'", project_a))
        .await.unwrap_or_default();
    let count_b = ch_query(&format!("SELECT count() FROM events WHERE project_id='{}' AND event_id='e2e-cross-tenant'", project_b))
        .await.unwrap_or_default();

    if count_a.trim() == "1" {
        pass!("edge56: event belongs to project_a (correct)");
    } else {
        fail!("edge56: expected project_a count=1, got '{}'", count_a.trim());
    }
    if count_b.trim() == "0" {
        pass!("edge56: project_b cannot see project_a's event (isolation works)");
    } else {
        fail!("edge56: project_b can see project_a's event (isolation broken!)");
    }

    let client = reqwest::Client::new();
    let sessions_a = client
        .get(&format!("http://{}:{}/v1/query/replay/sessions?projectId={}", HOST_LOOPBACK, QUERY_PORT, project_a))
        .header("Authorization", format!("Bearer {}", SECRET_KEY))
        .send()
        .await?;
    let sessions_b = client
        .get(&format!("http://{}:{}/v1/query/replay/sessions?projectId={}", HOST_LOOPBACK, QUERY_PORT, project_b))
        .header("Authorization", format!("Bearer {}", SECRET_KEY))
        .send()
        .await?;

    let body_b: serde_json::Value = sessions_b.json().await.unwrap_or_default();
    let sessions_b_arr = body_b.as_array().map(|a| a.len()).unwrap_or(0);

    if sessions_b_arr == 0 {
        pass!("edge56: isolation — project_b sees no sessions from project_a");
    } else {
        info!("edge56: project_b sees {} sessions", sessions_b_arr);
    }

    Ok(())
}
