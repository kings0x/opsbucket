use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::routes::admin;
use crate::SharedState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightResponse {
    id: Uuid,
    project_id: String,
    name: String,
    #[serde(rename = "type")]
    insight_type: String,
    spec: serde_json::Value,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightListResponse {
    insights: Vec<InsightResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInsightRequest {
    project_id: String,
    name: String,
    #[serde(rename = "type")]
    insight_type: String,
    spec: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightQueryParams {
    project_id: String,
}

pub async fn list_insights(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<InsightQueryParams>,
) -> Result<Json<InsightListResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        project_id: String,
        name: String,
        #[sqlx(rename = "type")]
        insight_type: String,
        spec: serde_json::Value,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, project_id, name, type, spec, created_at, updated_at FROM saved_insights WHERE project_id = $1 ORDER BY updated_at DESC",
    )
    .bind(&params.project_id)
    .fetch_all(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?;

    let insights = rows
        .into_iter()
        .map(|r| InsightResponse {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            insight_type: r.insight_type,
            spec: r.spec,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(InsightListResponse { insights }))
}

pub async fn create_insight(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateInsightRequest>,
) -> Result<Json<InsightResponse>, admin::AdminError> {
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
        "INSERT INTO saved_insights (id, project_id, name, type, spec, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(id)
    .bind(&req.project_id)
    .bind(&req.name)
    .bind(&req.insight_type)
    .bind(&req.spec)
    .bind(now)
    .bind(now)
    .execute(&state.pg)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to create insight");
        admin::internal_error()
    })?;

    Ok(Json(InsightResponse {
        id,
        project_id: req.project_id,
        name: req.name,
        insight_type: req.insight_type,
        spec: req.spec,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    }))
}

pub async fn delete_insight(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let result = sqlx::query("DELETE FROM saved_insights WHERE id = $1")
        .bind(id)
        .execute(&state.pg)
        .await
        .map_err(|_| admin::internal_error())?;

    if result.rows_affected() == 0 {
        return Err(admin::not_found());
    }

    Ok(Json(serde_json::json!({"status": "deleted"})))
}
