use std::sync::Arc;

use opsbucket_ingestion::{rate_limiter::RateLimiter, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let pg_pool = sqlx::postgres::PgPoolOptions::new()
        .connect(&std::env::var("DATABASE_URL")?)
        .await?;

    let redis_client = redis::Client::open(std::env::var("REDIS_URL")?)?;

    let kafka =
        opsbucket_ingestion::kafka::producer::KafkaProducer::new(&std::env::var("KAFKA_BROKERS")?)?;

    let rate_limiter = RateLimiter::new(1000, 1000);

    let state = Arc::new(AppState {
        redis: redis_client,
        pg: pg_pool,
        kafka,
        rate_limiter: std::sync::Mutex::new(rate_limiter),
    });

    let app = axum::Router::new()
        .route(
            "/v1/batch",
            axum::routing::post(opsbucket_ingestion::routes::batch::post_batch),
        )
        .route(
            "/health",
            axum::routing::get(opsbucket_ingestion::routes::health::get_health),
        )
        .with_state(state);

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);

    tracing::info!("listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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

    tracing::info!("shutdown signal received");
}
