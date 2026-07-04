use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

use opsbucket_query::config::Config;
use opsbucket_query::routes::{events, funnel, health, retention, segment};
use opsbucket_query::AppState;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.rust_log))
        .init();

    let redis_client = redis::Client::open(config.redis_url.clone())?;
    let redis = redis_client.get_connection_manager().await?;
    info!("redis connected");

    let pg = sqlx::PgPool::connect(&config.database_url).await?;
    info!("postgres connected");

    let ch = ::clickhouse::Client::default()
        .with_url(&config.clickhouse_url)
        .with_user(&config.clickhouse_user)
        .with_password(&config.clickhouse_password);
    info!("clickhouse client created");

    let state = Arc::new(AppState {
        secret_key: config.secret_key.clone(),
        pg,
        redis,
        ch_client: ch,
        config: config.clone(),
    });

    let app = Router::new()
        .route("/v1/query/funnel", post(funnel::handler))
        .route("/v1/query/retention", post(retention::handler))
        .route("/v1/query/segment", post(segment::handler))
        .route("/v1/query/events", get(events::handler))
        .route("/health", get(health::handler))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    info!("listening on {}", addr);

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

    info!("shutdown signal received");
}
