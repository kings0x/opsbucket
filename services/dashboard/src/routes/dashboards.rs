use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::routes::admin;
use crate::SharedState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WidgetResponse {
    id: Uuid,
    dashboard_id: Uuid,
    insight_id: Option<Uuid>,
    title: String,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    created_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResponse {
    id: Uuid,
    project_id: String,
    name: String,
    widgets: Vec<WidgetResponse>,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardSummary {
    id: Uuid,
    project_id: String,
    name: String,
    widget_count: i64,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardListResponse {
    dashboards: Vec<DashboardSummary>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateDashboardRequest {
    project_id: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDashboardRequest {
    name: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardQueryParams {
    project_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWidgetRequest {
    insight_id: Option<Uuid>,
    title: String,
    x: Option<i32>,
    y: Option<i32>,
    w: Option<i32>,
    h: Option<i32>,
}

async fn get_widgets(
    state: &SharedState,
    dashboard_id: Uuid,
) -> Result<Vec<WidgetResponse>, admin::AdminError> {
    #[derive(sqlx::FromRow)]
    struct WidgetRow {
        id: Uuid,
        dashboard_id: Uuid,
        insight_id: Option<Uuid>,
        title: String,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<WidgetRow> = sqlx::query_as(
        "SELECT id, dashboard_id, insight_id, title, x, y, w, h, created_at FROM dashboard_widgets WHERE dashboard_id = $1 ORDER BY y, x",
    )
    .bind(dashboard_id)
    .fetch_all(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?;

    Ok(rows
        .into_iter()
        .map(|r| WidgetResponse {
            id: r.id,
            dashboard_id: r.dashboard_id,
            insight_id: r.insight_id,
            title: r.title,
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            created_at: r.created_at.to_rfc3339(),
        })
        .collect())
}

pub async fn list_dashboards(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<DashboardQueryParams>,
) -> Result<Json<DashboardListResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        project_id: String,
        name: String,
        widget_count: i64,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT d.id, d.project_id, d.name, COUNT(w.id)::bigint AS widget_count, d.created_at, d.updated_at
         FROM dashboards d
         LEFT JOIN dashboard_widgets w ON w.dashboard_id = d.id
         WHERE d.project_id = $1
         GROUP BY d.id
         ORDER BY d.updated_at DESC",
    )
    .bind(&params.project_id)
    .fetch_all(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?;

    let dashboards = rows
        .into_iter()
        .map(|r| DashboardSummary {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            widget_count: r.widget_count,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(DashboardListResponse { dashboards }))
}

pub async fn create_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateDashboardRequest>,
) -> Result<Json<DashboardResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    if req.name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(admin::ErrorResponse {
                error: "validation_failed".into(),
            }),
        ));
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        "INSERT INTO dashboards (id, project_id, name, created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(&req.project_id)
    .bind(&req.name)
    .bind(now)
    .bind(now)
    .execute(&state.pg)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to create dashboard");
        admin::internal_error()
    })?;

    Ok(Json(DashboardResponse {
        id,
        project_id: req.project_id,
        name: req.name,
        widgets: vec![],
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    }))
}

pub async fn get_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<DashboardResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        project_id: String,
        name: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let row: Row = sqlx::query_as(
        "SELECT id, project_id, name, created_at, updated_at FROM dashboards WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?
    .ok_or_else(admin::not_found)?;

    let widgets = get_widgets(&state, id).await?;

    Ok(Json(DashboardResponse {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        widgets,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }))
}

pub async fn update_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateDashboardRequest>,
) -> Result<Json<serde_json::Value>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let result = sqlx::query("UPDATE dashboards SET name = $1, updated_at = now() WHERE id = $2")
        .bind(&req.name)
        .bind(id)
        .execute(&state.pg)
        .await
        .map_err(|_| admin::internal_error())?;

    if result.rows_affected() == 0 {
        return Err(admin::not_found());
    }

    Ok(Json(serde_json::json!({"status": "updated"})))
}

pub async fn delete_dashboard(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let result = sqlx::query("DELETE FROM dashboards WHERE id = $1")
        .bind(id)
        .execute(&state.pg)
        .await
        .map_err(|_| admin::internal_error())?;

    if result.rows_affected() == 0 {
        return Err(admin::not_found());
    }

    Ok(Json(serde_json::json!({"status": "deleted"})))
}

pub async fn add_widget(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(dashboard_id): Path<Uuid>,
    Json(req): Json<AddWidgetRequest>,
) -> Result<Json<WidgetResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let exists: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM dashboards WHERE id = $1")
        .bind(dashboard_id)
        .fetch_optional(&state.pg)
        .await
        .map_err(|_| admin::internal_error())?;

    if exists.is_none() {
        return Err(admin::not_found());
    }

    let id = Uuid::new_v4();
    let now = chrono::Utc::now();

    sqlx::query(
        "INSERT INTO dashboard_widgets (id, dashboard_id, insight_id, title, x, y, w, h, created_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
    )
    .bind(id)
    .bind(dashboard_id)
    .bind(req.insight_id)
    .bind(&req.title)
    .bind(req.x.unwrap_or(0))
    .bind(req.y.unwrap_or(0))
    .bind(req.w.unwrap_or(2))
    .bind(req.h.unwrap_or(2))
    .bind(now)
    .execute(&state.pg)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to add widget");
        admin::internal_error()
    })?;

    Ok(Json(WidgetResponse {
        id,
        dashboard_id,
        insight_id: req.insight_id,
        title: req.title,
        x: req.x.unwrap_or(0),
        y: req.y.unwrap_or(0),
        w: req.w.unwrap_or(2),
        h: req.h.unwrap_or(2),
        created_at: now.to_rfc3339(),
    }))
}

pub async fn remove_widget(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((_dashboard_id, widget_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let result = sqlx::query("DELETE FROM dashboard_widgets WHERE id = $1")
        .bind(widget_id)
        .execute(&state.pg)
        .await
        .map_err(|_| admin::internal_error())?;

    if result.rows_affected() == 0 {
        return Err(admin::not_found());
    }

    Ok(Json(serde_json::json!({"status": "deleted"})))
}
