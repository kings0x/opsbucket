use std::sync::Arc;
use std::time::Duration;

use axum::routing::{delete, get, post, put};
use axum::Router;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::Config;
use crate::db;
use crate::routes::{admin, auth, cohorts, dashboards, insights, query_proxy, secret_keys};
use crate::spa;
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
            query_secret_key: self.config.query_secret_key.clone(),
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
                "/api/admin/projects/:project_id",
                delete(admin::delete_project),
            )
            .route(
                "/api/admin/projects/:project_id/write-keys",
                get(admin::get_project_write_keys),
            )
            .route(
                "/api/admin/projects/:project_id/write-keys",
                post(admin::create_write_key),
            )
            .route(
                "/api/admin/projects/:project_id/write-keys/:key_id",
                delete(admin::revoke_write_key),
            )
            .route("/api/admin/health", get(admin::health))
            // Secret keys CRUD
            .route("/api/admin/secret-keys", get(secret_keys::list_secret_keys))
            .route("/api/admin/secret-keys", post(secret_keys::create_secret_key))
            .route(
                "/api/admin/secret-keys/:key_id",
                delete(secret_keys::revoke_secret_key),
            )
            // Query proxy (admin auth protects these)
            .route(
                "/api/admin/query/funnel",
                post(query_proxy::funnel_handler),
            )
            .route(
                "/api/admin/query/retention",
                post(query_proxy::retention_handler),
            )
            .route(
                "/api/admin/query/segment",
                post(query_proxy::segment_handler),
            )
            .route(
                "/api/admin/query/events",
                get(query_proxy::events_handler),
            )
            .route(
                "/api/admin/query/schema",
                get(query_proxy::schema_handler),
            )
            .route(
                "/api/admin/query/stats",
                get(query_proxy::stats_handler),
            )
            // Cohorts CRUD
            .route("/api/admin/cohorts", get(cohorts::list_cohorts))
            .route("/api/admin/cohorts", post(cohorts::create_cohort))
            .route(
                "/api/admin/cohorts/:id",
                get(cohorts::get_cohort),
            )
            .route(
                "/api/admin/cohorts/:id",
                put(cohorts::update_cohort),
            )
            .route(
                "/api/admin/cohorts/:id",
                delete(cohorts::delete_cohort),
            )
            // Insights CRUD
            .route("/api/admin/insights", get(insights::list_insights))
            .route("/api/admin/insights", post(insights::create_insight))
            .route(
                "/api/admin/insights/:id",
                delete(insights::delete_insight),
            )
            // Dashboards CRUD
            .route(
                "/api/admin/dashboards",
                get(dashboards::list_dashboards),
            )
            .route(
                "/api/admin/dashboards",
                post(dashboards::create_dashboard),
            )
            .route(
                "/api/admin/dashboards/:id",
                get(dashboards::get_dashboard),
            )
            .route(
                "/api/admin/dashboards/:id",
                put(dashboards::update_dashboard),
            )
            .route(
                "/api/admin/dashboards/:id",
                delete(dashboards::delete_dashboard),
            )
            .route(
                "/api/admin/dashboards/:id/widgets",
                post(dashboards::add_widget),
            )
            .route(
                "/api/admin/dashboards/:id/widgets/:wid",
                delete(dashboards::remove_widget),
            )
            .fallback(spa::handler)
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
