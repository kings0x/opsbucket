use std::sync::Arc;
use std::time::Duration;

use axum::routing::{delete, get, post};
use axum::Router;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::Config;
use crate::db;
use crate::routes::{admin, auth};
use crate::AppState;

pub struct Server {
    config: Config,
}

impl Server {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub async fn run(&self) -> anyhow::Result<()> {
        let pg = db::init_pg(&self.config.database_url).await?;
        let redis = db::init_redis(&self.config.redis_url).await?;

        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;

        let state = Arc::new(AppState {
            pg,
            redis,
            http_client,
            query_url: self.config.query_url.clone(),
            session_ttl_seconds: self.config.session_ttl_seconds,
        });

        let app = Router::new()
            .route("/api/admin/setup", post(auth::setup))
            .route("/api/admin/login", post(auth::login))
            .route("/api/admin/logout", post(auth::logout))
            .route("/api/admin/me", get(auth::me))
            .route("/api/admin/projects", get(admin::list_projects))
            .route("/api/admin/projects", post(admin::create_project))
            .route(
                "/api/admin/projects/{project_id}",
                delete(admin::delete_project),
            )
            .route(
                "/api/admin/projects/{project_id}/write-keys",
                get(admin::get_project_write_keys),
            )
            .route(
                "/api/admin/projects/{project_id}/write-keys",
                post(admin::create_write_key),
            )
            .route(
                "/api/admin/projects/{project_id}/write-keys/{key_id}",
                delete(admin::revoke_write_key),
            )
            .route("/api/admin/health", get(admin::health))
            .layer(TraceLayer::new_for_http())
            .layer(CorsLayer::permissive())
            .with_state(state);

        let addr = format!("0.0.0.0:{}", self.config.port);
        info!("dashboard listening on {addr}");

        let listener = TcpListener::bind(&addr).await?;
        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        Ok(())
    }
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::select! {
        _ = ctrl_c => { info!("received SIGINT"); }
    }

    info!("shutting down");
    tokio::time::sleep(Duration::from_millis(100)).await;
}
