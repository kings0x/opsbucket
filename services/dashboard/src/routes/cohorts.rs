use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::routes::admin;
use crate::SharedState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CohortResponse {
    id: Uuid,
    project_id: String,
    name: String,
    conditions: serde_json::Value,
    created_at: String,
    updated_at: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CohortListResponse {
    cohorts: Vec<CohortResponse>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCohortRequest {
    project_id: String,
    name: String,
    conditions: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCohortRequest {
    name: String,
    conditions: serde_json::Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CohortQueryParams {
    project_id: String,
}

pub async fn list_cohorts(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(params): Query<CohortQueryParams>,
) -> Result<Json<CohortListResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        project_id: String,
        name: String,
        conditions: serde_json::Value,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, project_id, name, conditions, created_at, updated_at FROM saved_cohorts WHERE project_id = $1 ORDER BY updated_at DESC",
    )
    .bind(&params.project_id)
    .fetch_all(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?;

    let cohorts = rows
        .into_iter()
        .map(|r| CohortResponse {
            id: r.id,
            project_id: r.project_id,
            name: r.name,
            conditions: r.conditions,
            created_at: r.created_at.to_rfc3339(),
            updated_at: r.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(CohortListResponse { cohorts }))
}

pub async fn create_cohort(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateCohortRequest>,
) -> Result<Json<CohortResponse>, admin::AdminError> {
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
        "INSERT INTO saved_cohorts (id, project_id, name, conditions, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(&req.project_id)
    .bind(&req.name)
    .bind(&req.conditions)
    .bind(now)
    .bind(now)
    .execute(&state.pg)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to create cohort");
        admin::internal_error()
    })?;

    Ok(Json(CohortResponse {
        id,
        project_id: req.project_id,
        name: req.name,
        conditions: req.conditions,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
    }))
}

pub async fn get_cohort(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<CohortResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        project_id: String,
        name: String,
        conditions: serde_json::Value,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let row: Row = sqlx::query_as(
        "SELECT id, project_id, name, conditions, created_at, updated_at FROM saved_cohorts WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?
    .ok_or_else(admin::not_found)?;

    Ok(Json(CohortResponse {
        id: row.id,
        project_id: row.project_id,
        name: row.name,
        conditions: row.conditions,
        created_at: row.created_at.to_rfc3339(),
        updated_at: row.updated_at.to_rfc3339(),
    }))
}

pub async fn update_cohort(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCohortRequest>,
) -> Result<Json<CohortResponse>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let now = chrono::Utc::now();
    let result = sqlx::query(
        "UPDATE saved_cohorts SET name = $1, conditions = $2, updated_at = $3 WHERE id = $4",
    )
    .bind(&req.name)
    .bind(&req.conditions)
    .bind(now)
    .bind(id)
    .execute(&state.pg)
    .await
    .map_err(|_| admin::internal_error())?;

    if result.rows_affected() == 0 {
        return Err(admin::not_found());
    }

    get_cohort(State(state), headers, Path(id)).await
}

pub async fn delete_cohort(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, admin::AdminError> {
    admin::admin_auth(&state, &headers).await?;

    let result = sqlx::query("DELETE FROM saved_cohorts WHERE id = $1")
        .bind(id)
        .execute(&state.pg)
        .await
        .map_err(|_| admin::internal_error())?;

    if result.rows_affected() == 0 {
        return Err(admin::not_found());
    }

    Ok(Json(serde_json::json!({"status": "deleted"})))
}
