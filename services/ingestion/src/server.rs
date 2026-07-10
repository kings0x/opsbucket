use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderValue, Method};
use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::auth::{AuthValidator, RedisPgAuth};
use crate::config::Config;
use crate::db;
use crate::kafka::producer::KafkaProducer;
use crate::kafka::KafkaHealth;
use crate::rate_limiter::RateLimiter;
use crate::routes::{batch, health, replay};
use crate::AppState;

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let pg = db::postgres::init_pool(&self.config.database_url).await?;
        db::postgres::check_health(&pg).await?;
        info!("postgres connected and healthy");

        let redis = db::redis::init_pool(&self.config.redis_url)?;
        db::redis::check_health(&redis).await?;
        info!("redis connected and healthy");

        let kafka = Arc::new(KafkaProducer::new(&self.config.kafka_brokers)?);
        let kafka_health: Arc<dyn KafkaHealth> = kafka.clone();
        kafka_health.check_health().await?;
        info!("kafka connected and healthy");

        let auth: Arc<dyn AuthValidator> = Arc::new(RedisPgAuth::new(redis.clone(), pg.clone()));

        let rate_limiter = RateLimiter::new(
            self.config.rate_limit_capacity,
            self.config.rate_limit_refill,
        );

        let replay_rate_limiter = RateLimiter::new(
            self.config.rate_limit_capacity,
            self.config.rate_limit_refill,
        );

        let state = Arc::new(AppState {
            auth,
            kafka,
            kafka_health,
            pg: Arc::new(pg),
            redis: Arc::new(redis),
            rate_limiter: std::sync::Mutex::new(rate_limiter),
            replay_rate_limiter: std::sync::Mutex::new(replay_rate_limiter),
            trust_proxy_headers: self.config.trust_proxy_headers,
        });

        let app = Router::new()
            .route("/v1/batch", post(batch::post_batch))
            .route("/capture/replay", post(replay::post_replay))
            .route("/health", get(health::get_health))
            .layer(DefaultBodyLimit::max(1_048_576))
            .layer(cors_layer(&self.config.cors_allowed_origins))
            .layer(TraceLayer::new_for_http())
            .with_state(state);

        let addr = format!("0.0.0.0:{}", self.config.port);
        info!("listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

fn cors_layer(allowed_origins: &[String]) -> CorsLayer {
    if allowed_origins.is_empty() {
        return CorsLayer::permissive();
    }

    let origins = allowed_origins
        .iter()
        .map(|origin| {
            HeaderValue::from_str(origin)
                .unwrap_or_else(|_| panic!("invalid CORS origin: {}", origin))
        })
        .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::POST, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("shutdown signal received");
}
