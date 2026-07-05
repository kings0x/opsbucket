//! Comprehensive E2E and integration tests for the OpsBucket Query service.
//!
//! Requires: ClickHouse, Postgres, Redis running via Docker Compose.
//! Run: cargo test -p opsbucket-query --test e2e -- --test-threads=1

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Method, Request, StatusCode};
use chrono::{Duration, Utc};
use common::*;
use opsbucket_query::AppState;
use serde_json::Value;
use tower::ServiceExt;

type SetupResult = (Arc<AppState>, clickhouse::Client);

// ────────────────────────────────────────────────────────────────────
// Setup / Teardown
// ────────────────────────────────────────────────────────────────────

async fn setup() -> SetupResult {
    let ch = clickhouse_client();
    let pg = pg_pool().await;
    clean_clickhouse(&ch).await;
    clean_identity(&pg).await;
    insert_funnel_fixtures(&ch).await;
    insert_retention_fixtures(&ch).await;
    insert_page_event_fixtures(&ch).await;
    insert_identity_fixtures(&pg).await;
    let state = build_test_state(None).await;
    (state, ch)
}

async fn teardown(ch: &clickhouse::Client, state: &Arc<AppState>) {
    clean_clickhouse(ch).await;
    clean_identity(&state.pg).await;
    let mut redis = state.redis.clone();
    clean_redis(&mut redis).await;
}

fn auth_header() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer test-secret-key"),
    );
    headers
}

fn bad_auth_header() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "authorization",
        HeaderValue::from_static("Bearer wrong-key"),
    );
    headers
}

fn json_body(s: &str) -> Body {
    Body::from(s.to_owned())
}

fn empty_body() -> Body {
    Body::empty()
}

async fn do_request(
    app: &mut axum::Router,
    method: Method,
    uri: &str,
    headers: HeaderMap,
    body_val: Body,
) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(uri);
    for (k, v) in headers.iter() {
        req = req.header(k.as_str(), v.to_str().unwrap());
    }
    let req = req.body(body_val).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 1_000_000)
        .await
        .unwrap();
    let val: Value = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    (status, val)
}

async fn do_post(
    app: &mut axum::Router,
    uri: &str,
    headers: HeaderMap,
    request_body: &str,
) -> (StatusCode, Value) {
    do_request(app, Method::POST, uri, headers, json_body(request_body)).await
}

async fn do_get(app: &mut axum::Router, uri: &str, headers: HeaderMap) -> (StatusCode, Value) {
    do_request(app, Method::GET, uri, headers, empty_body()).await
}

fn funnel_body(project_id: &str, steps: &[&str], window: u64, start: &str, end: &str) -> String {
    serde_json::json!({
        "projectId": project_id,
        "steps": steps,
        "windowSeconds": window,
        "dateRange": { "start": start, "end": end }
    })
    .to_string()
}

fn retention_body(
    project_id: &str,
    event_name: &str,
    interval: &str,
    periods: u64,
    start: &str,
    end: &str,
) -> String {
    serde_json::json!({
        "projectId": project_id,
        "eventName": event_name,
        "interval": interval,
        "periods": periods,
        "dateRange": { "start": start, "end": end }
    })
    .to_string()
}

fn segment_body(project_id: &str, conditions: Value, limit: u64) -> String {
    serde_json::json!({
        "projectId": project_id,
        "conditions": conditions,
        "limit": limit
    })
    .to_string()
}

// ────────────────────────────────────────────────────────────────────
// AUTH TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_auth_valid_key() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, _) = do_post(
        &mut app,
        "/v1/query/funnel",
        auth_header(),
        &funnel_body(
            PROJ_1,
            &["x"],
            3600,
            "2026-06-01T00:00:00Z",
            "2026-06-20T00:00:00Z",
        ),
    )
    .await;
    assert_ne!(status, StatusCode::UNAUTHORIZED);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_auth_invalid_key() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, val) = do_post(
        &mut app,
        "/v1/query/funnel",
        bad_auth_header(),
        &funnel_body(
            PROJ_1,
            &["x"],
            3600,
            "2026-06-01T00:00:00Z",
            "2026-06-20T00:00:00Z",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(val["error"], "unauthorized");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_auth_missing_key() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, val) = do_post(
        &mut app,
        "/v1/query/funnel",
        HeaderMap::new(),
        &funnel_body(
            PROJ_1,
            &["x"],
            3600,
            "2026-06-01T00:00:00Z",
            "2026-06-20T00:00:00Z",
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(val["error"], "unauthorized");
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// FUNNEL TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_funnel_single_step() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_1,
        &["Page Viewed"],
        3600,
        "2026-06-14T00:00:00Z",
        "2026-06-21T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let steps = val["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0]["name"], "Page Viewed");
    assert!(
        steps[0]["users"].as_u64().unwrap() >= 5,
        ">=5 users viewed page: {:?}",
        steps
    );
    assert!((steps[0]["conversionRate"].as_f64().unwrap() - 1.0).abs() < 0.001);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_funnel_multi_step() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_1,
        &["Page Viewed", "Button Clicked", "Account Created"],
        3600,
        "2026-06-14T00:00:00Z",
        "2026-06-21T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let steps = val["steps"].as_array().unwrap();
    assert_eq!(steps.len(), 3);

    assert_eq!(steps[0]["name"], "Page Viewed");
    let s1 = steps[0]["users"].as_u64().unwrap();
    assert!(s1 >= 5, ">=5: {}", s1);

    assert_eq!(steps[1]["name"], "Button Clicked");
    let s2 = steps[1]["users"].as_u64().unwrap();
    assert_eq!(s2, 3, "3 clicked: {}", s2);

    assert_eq!(steps[2]["name"], "Account Created");
    let s3 = steps[2]["users"].as_u64().unwrap();
    assert_eq!(s3, 1, "1 within 3600s window: {}", s3);

    assert!((steps[0]["conversionRate"].as_f64().unwrap() - 1.0).abs() < 0.001);
    let conv_12 = steps[1]["conversionRate"].as_f64().unwrap();
    assert!(
        (conv_12 - (3.0_f64 / s1 as f64)).abs() < 0.001,
        "conv 1->2: {}",
        conv_12
    );
    let conv_23 = steps[2]["conversionRate"].as_f64().unwrap();
    assert!(
        (conv_23 - (1.0_f64 / 3.0_f64)).abs() < 0.001,
        "conv 2->3: {}",
        conv_23
    );
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_funnel_empty_results() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_1,
        &["Page Viewed"],
        3600,
        "2020-01-01T00:00:00Z",
        "2020-01-02T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["steps"][0]["users"], 0);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_funnel_project_isolation() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_2,
        &["Page Viewed"],
        3600,
        "2026-06-14T00:00:00Z",
        "2026-06-21T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["steps"][0]["users"], 0);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_funnel_large_window() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_1,
        &["Page Viewed", "Button Clicked", "Account Created"],
        7200,
        "2026-06-14T00:00:00Z",
        "2026-06-21T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        val["steps"][2]["users"].as_u64().unwrap(),
        2,
        "anon_a + anon_d with 7200s window"
    );
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// RETENTION TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_retention_weekly() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = retention_body(
        PROJ_2,
        "App Opened",
        "week",
        4,
        "2026-06-01T00:00:00Z",
        "2026-06-28T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/retention", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let cohorts = val["cohorts"].as_array().unwrap();

    let dates: Vec<&str> = cohorts
        .iter()
        .map(|c| c["cohortDate"].as_str().unwrap())
        .collect();
    assert!(dates.contains(&"2026-06-01"), "june 1 cohort: {:?}", dates);
    assert!(dates.contains(&"2026-06-08"), "june 8 cohort: {:?}", dates);

    let c1 = cohorts
        .iter()
        .find(|c| c["cohortDate"] == "2026-06-01")
        .unwrap();
    assert_eq!(
        c1["initialUsers"], 3,
        "3 users week 1: {}",
        c1["initialUsers"]
    );
    let periods = c1["periods"].as_array().unwrap();
    assert_eq!(periods[0]["users"].as_u64().unwrap(), 3);
    assert!((periods[0]["rate"].as_f64().unwrap() - 1.0).abs() < 0.001);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_retention_empty_project() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = retention_body(
        "nonexistent_proj",
        "App Opened",
        "week",
        4,
        "2026-06-01T00:00:00Z",
        "2026-06-28T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/retention", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["cohorts"].as_array().unwrap().len(), 0);
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// SEGMENT TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_segment_event_count_gt() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([{"type":"event_count","eventName":"Feature Used","op":"gt","value":3,"withinDays":30}]);
    let body = segment_body(PROJ_1, cond, 100);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let users = val["users"].as_array().unwrap();
    assert!(!users.is_empty());
    assert!(
        users.iter().any(|u| u == "anon_f"),
        "anon_f should match: {:?}",
        users
    );
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_segment_event_count_eq() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([{"type":"event_count","eventName":"Feature Used","op":"eq","value":2,"withinDays":30}]);
    let body = segment_body(PROJ_1, cond, 100);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    assert!(val["users"]
        .as_array()
        .unwrap()
        .iter()
        .any(|u| u == "anon_h"));
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_segment_no_results() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([{"type":"event_count","eventName":"Feature Used","op":"gt","value":10,"withinDays":30}]);
    let body = segment_body(PROJ_1, cond, 100);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(val["users"].as_array().unwrap().len(), 0);
    assert_eq!(val["total"], 0);
    assert!(!val["truncated"].as_bool().unwrap());
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_segment_truncated() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([{"type":"event_count","eventName":"Feature Used","op":"gt","value":0,"withinDays":30}]);
    let body = segment_body(PROJ_1, cond, 1);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    assert_eq!(val["users"].as_array().unwrap().len(), 1);
    assert!(val["truncated"].as_bool().unwrap());
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_segment_multi_condition_and() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([
        {"type":"event_count","eventName":"Feature Used","op":"gt","value":0,"withinDays":30},
        {"type":"trait","key":"plan","op":"eq","value":"pro"}
    ]);
    let body = segment_body(PROJ_1, cond, 100);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let users = val["users"].as_array().unwrap();
    assert!(!users.is_empty());
    assert!(
        users.iter().any(|u| u == "usr_g" || u == "anon_g"),
        "usr_g should match: {:?}",
        users
    );
    assert!(
        !users.iter().any(|u| *u == "anon_f"),
        "anon_f should NOT match"
    );
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// RAW EVENTS TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_events_pagination() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, val) = do_get(
        &mut app,
        &format!("/v1/query/events?projectId={}&limit=3", PROJ_1),
        auth_header(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let events = val["events"].as_array().unwrap();
    assert!(!events.is_empty());
    assert_eq!(events.len(), 3);
    for i in 1..events.len() {
        assert!(events[i - 1]["timestamp"].as_str() >= events[i]["timestamp"].as_str());
    }
    assert!(val["next_cursor"].is_string());

    let cursor = val["next_cursor"].as_str().unwrap();
    let page1_ids: Vec<&str> = events
        .iter()
        .map(|e| e["eventId"].as_str().unwrap())
        .collect();
    let (status, val2) = do_get(
        &mut app,
        &format!(
            "/v1/query/events?projectId={}&limit=3&cursor={}",
            PROJ_1, cursor
        ),
        auth_header(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "page2: {}", val2);
    for e in val2["events"].as_array().unwrap() {
        assert!(!page1_ids.contains(&e["eventId"].as_str().unwrap()));
    }
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_events_event_name_filter() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, val) = do_get(
        &mut app,
        &format!(
            "/v1/query/events?projectId={}&eventName=Signup+Flow&limit=10",
            PROJ_1
        ),
        auth_header(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    assert_eq!(val["events"].as_array().unwrap().len(), 2);
    for e in val["events"].as_array().unwrap() {
        assert_eq!(e["eventName"], "Signup Flow");
    }
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_events_user_id_filter() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, val) = do_get(
        &mut app,
        &format!(
            "/v1/query/events?projectId={}&userId=usr_query&limit=10",
            PROJ_1
        ),
        auth_header(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    assert!(!val["events"].as_array().unwrap().is_empty());
    for e in val["events"].as_array().unwrap() {
        assert_eq!(e["userId"].as_str().unwrap_or(""), "usr_query");
    }
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_events_identity_join() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());

    let ch_client = clickhouse_client();
    let base = base_ts();
    let row = ChEventRow::new(
        PROJ_1,
        "ev_missing1",
        "Page Viewed",
        "page",
        "anon_missing",
        None,
        base,
        Default::default(),
    );
    let mut inserter = ch_client.insert("events").expect("create inserter");
    inserter.write(&row).await.expect("write");
    inserter.end().await.expect("flush");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let (status, val) = do_get(
        &mut app,
        &format!(
            "/v1/query/events?projectId={}&userId=usr_resolved_late&limit=10",
            PROJ_1
        ),
        auth_header(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "body: {}", val);
    let resolved = val["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["userId"] == "usr_resolved_late");
    assert!(resolved, "identity join should resolve: {:?}", val);

    ch_client
        .query("DELETE FROM events WHERE event_id = 'ev_missing1'")
        .execute()
        .await
        .ok();
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// HEALTH CHECK
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_health() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, _) = do_get(&mut app, "/health", HeaderMap::new()).await;
    assert_eq!(status, StatusCode::OK);
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// VALIDATION / ERROR TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_validation_empty_body() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, _) = do_post(&mut app, "/v1/query/funnel", auth_header(), "").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_malformed_json() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let (status, _) = do_post(
        &mut app,
        "/v1/query/funnel",
        auth_header(),
        "not valid json",
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_future_date() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let future = Utc::now() + Duration::days(100);
    let body = funnel_body(
        PROJ_1,
        &["x"],
        3600,
        "2026-06-01T00:00:00Z",
        &future.to_rfc3339(),
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_end_before_start() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_1,
        &["x"],
        3600,
        "2026-06-20T00:00:00Z",
        "2026-06-01T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_too_many_steps() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let steps: Vec<&str> = (0..21).map(|_| "step").collect();
    let body = funnel_body(
        PROJ_1,
        &steps,
        3600,
        "2026-06-01T00:00:00Z",
        "2026-06-20T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_empty_project_id() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        "",
        &["x"],
        3600,
        "2026-06-01T00:00:00Z",
        "2026-06-20T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_invalid_interval() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = retention_body(
        PROJ_1,
        "App Opened",
        "month",
        4,
        "2026-06-01T00:00:00Z",
        "2026-06-28T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/retention", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_unsupported_operator() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([{"type":"event_count","eventName":"Feature Used","op":"bad_op","value":3,"withinDays":30}]);
    let body = segment_body(PROJ_1, cond, 100);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_too_many_conditions() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let mut conds = vec![];
    for i in 0..11 {
        conds.push(serde_json::json!({"type":"event_count","eventName":format!("e{}",i),"op":"gt","value":1,"withinDays":30}));
    }
    let body = segment_body(PROJ_1, serde_json::json!(conds), 100);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_excessive_date_range() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let body = funnel_body(
        PROJ_1,
        &["x"],
        3600,
        "2025-06-01T00:00:00Z",
        "2026-06-20T00:00:00Z",
    );
    let (status, val) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

#[tokio::test]
async fn test_validation_segment_limit_exceeds_max() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());
    let cond = serde_json::json!([{"type":"event_count","eventName":"x","op":"gt","value":1}]);
    let body = segment_body(PROJ_1, cond, 20000);
    let (status, val) = do_post(&mut app, "/v1/query/segment", auth_header(), &body).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(val["error"], "invalid_request");
    teardown(&ch, &state).await;
}

// ────────────────────────────────────────────────────────────────────
// CACHE TESTS
// ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_cache_funnel() {
    let (state, ch) = setup().await;
    let mut app = build_router(state.clone());

    let body = funnel_body(
        PROJ_1,
        &["Page Viewed"],
        3600,
        "2026-06-14T00:00:00Z",
        "2026-06-21T00:00:00Z",
    );

    let (status1, val1) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status1, StatusCode::OK);

    let (status2, val2) = do_post(&mut app, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status2, StatusCode::OK);
    assert_eq!(val1["steps"], val2["steps"], "cached result matches");

    // Verify from a separate router (shared Redis)
    let state2 = build_test_state(None).await;
    let mut app2 = build_router(state2.clone());
    let (status3, val3) = do_post(&mut app2, "/v1/query/funnel", auth_header(), &body).await;
    assert_eq!(status3, StatusCode::OK);
    assert_eq!(val1["steps"], val3["steps"], "cross-router cache hit");

    teardown(&ch, &state).await;
}
