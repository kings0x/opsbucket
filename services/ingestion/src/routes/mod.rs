pub mod batch;
pub mod health;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::extract::DefaultBodyLimit;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use tower::util::ServiceExt;

    use crate::auth::MockAuth;
    use crate::kafka::producer::MockProducer;
    use crate::rate_limiter::RateLimiter;
    use crate::AppState;

    fn test_app(auth_keys: Vec<(&str, &str)>, rate_capacity: u64) -> Router {
        let auth = MockAuth::new();
        for (key, pid) in auth_keys {
            auth.insert(key, pid);
        }

        let kafka = MockProducer::new();
        let rate_limiter = RateLimiter::new(rate_capacity, rate_capacity);

        let state = Arc::new(AppState {
            auth: Arc::new(auth),
            kafka: Arc::new(kafka),
            rate_limiter: std::sync::Mutex::new(rate_limiter),
        });

        Router::new()
            .route("/v1/batch", axum::routing::post(super::batch::post_batch))
            .route("/health", axum::routing::get(super::health::get_health))
            .layer(DefaultBodyLimit::max(1_048_576))
            .with_state(state)
    }

    fn track_body() -> Vec<u8> {
        serde_json::json!({
            "sentAt": "2026-06-29T10:00:00.000Z",
            "batch": [{
                "messageId": "msg-1",
                "type": "track",
                "anonymousId": "anon-1",
                "userId": null,
                "originalTimestamp": "2026-06-29T10:00:00.000Z",
                "event": "Button Clicked",
                "properties": {},
                "context": {
                    "library": { "name": "test", "version": "1.0" },
                    "page": {
                        "url": "https://example.com",
                        "path": "/",
                        "referrer": "",
                        "title": "Test",
                        "search": ""
                    },
                    "screen": { "width": 1920, "height": 1080, "density": 1.0 },
                    "userAgent": "test",
                    "locale": "en-US",
                    "timezone": "UTC",
                    "campaign": {
                        "source": null, "medium": null, "name": null,
                        "term": null, "content": null
                    }
                }
            }]
        })
        .to_string()
        .into_bytes()
    }

    #[tokio::test]
    async fn health_returns_200() {
        let app = test_app(vec![], 100);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn full_successful_flow() {
        let app = test_app(vec![("test-key", "proj-1")], 100);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch")
                    .header("authorization", "Bearer test-key")
                    .header("content-type", "application/json")
                    .body(Body::from(track_body()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn invalid_write_key_returns_401() {
        let app = test_app(vec![], 100);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch")
                    .header("authorization", "Bearer bad-key")
                    .header("content-type", "application/json")
                    .body(Body::from(track_body()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn missing_write_key_returns_401() {
        let app = test_app(vec![], 100);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch")
                    .header("content-type", "application/json")
                    .body(Body::from(track_body()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn rate_limited_returns_429() {
        let app = test_app(vec![("test-key", "proj-1")], 0);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch")
                    .header("authorization", "Bearer test-key")
                    .header("content-type", "application/json")
                    .body(Body::from(track_body()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn malformed_body_returns_400() {
        let app = test_app(vec![("test-key", "proj-1")], 100);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch")
                    .header("authorization", "Bearer test-key")
                    .header("content-type", "application/json")
                    .body(Body::from("not valid json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn write_key_from_query_param() {
        let app = test_app(vec![("query-key", "proj-1")], 100);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch?write_key=query-key")
                    .header("content-type", "application/json")
                    .body(Body::from(track_body()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn kafka_failure_returns_502() {
        let auth = MockAuth::new();
        auth.insert("test-key", "proj-1");

        let kafka = MockProducer::new();
        *kafka.fail_on_send.lock().unwrap() = true;

        let rate_limiter = RateLimiter::new(100, 100);

        let state = Arc::new(AppState {
            auth: Arc::new(auth),
            kafka: Arc::new(kafka),
            rate_limiter: std::sync::Mutex::new(rate_limiter),
        });

        let app = Router::new()
            .route("/v1/batch", axum::routing::post(super::batch::post_batch))
            .layer(DefaultBodyLimit::max(1_048_576))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/batch")
                    .header("authorization", "Bearer test-key")
                    .header("content-type", "application/json")
                    .body(Body::from(track_body()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);
    }
}
