use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::routes::auth;
use crate::SharedState;

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

type AdminError = (StatusCode, Json<ErrorResponse>);

#[derive(Serialize)]
pub struct ProjectResponse {
    id: String,
    name: String,
    created_at: String,
}

#[derive(Serialize)]
pub struct ProjectListResponse {
    projects: Vec<ProjectResponse>,
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    name: String,
}

#[derive(Serialize)]
pub struct CreateProjectResponse {
    id: String,
    name: String,
    write_key: String,
    created_at: String,
}

#[derive(Serialize)]
pub struct WriteKeyResponse {
    id: Uuid,
    key: String,
    created_at: String,
    revoked_at: Option<String>,
}

#[derive(Serialize)]
pub struct WriteKeyListResponse {
    write_keys: Vec<WriteKeyResponse>,
}

#[derive(Serialize)]
pub struct HealthCheckResponse {
    status: String,
    checks: serde_json::Value,
}

fn internal_error() -> AdminError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal_error".into(),
        }),
    )
}

fn not_found() -> AdminError {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not_found".into(),
        }),
    )
}

async fn admin_auth(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<(Uuid, String), AdminError> {
    auth::get_session(state, headers)
        .await
        .map_err(|(code, json)| {
            let err = json.0;
            (code, Json(ErrorResponse { error: err.error }))
        })
}

pub async fn list_projects(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<ProjectListResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct ProjectRow {
        id: String,
        name: String,
        created_at: chrono::DateTime<chrono::Utc>,
    }

    let rows: Vec<ProjectRow> = sqlx::query_as(
        "SELECT id, name, created_at FROM projects ORDER BY created_at DESC",
    )
    .fetch_all(&state.pg)
    .await
    .map_err(|_| internal_error())?;

    let projects = rows
        .into_iter()
        .map(|r| ProjectResponse {
            id: r.id,
            name: r.name,
            created_at: r.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ProjectListResponse { projects }))
}

pub async fn create_project(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateProjectRequest>,
) -> Result<Json<CreateProjectResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    if req.name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_failed".into(),
            }),
        ));
    }

    let project_id = format!("proj_{}", Uuid::new_v4().to_string().replace('-', ""));
    let write_key = format!("wk_{}", Uuid::new_v4().to_string().replace('-', ""));

    sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2)")
        .bind(&project_id)
        .bind(&req.name)
        .execute(&state.pg)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create project");
            internal_error()
        })?;

    sqlx::query("INSERT INTO write_keys (project_id, key) VALUES ($1, $2)")
        .bind(&project_id)
        .bind(&write_key)
        .execute(&state.pg)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create write key");
            internal_error()
        })?;

    Ok(Json(CreateProjectResponse {
        id: project_id,
        name: req.name,
        write_key,
        created_at: chrono::Utc::now().to_rfc3339(),
    }))
}

pub async fn delete_project(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<serde_json::Value>, AdminError> {
    admin_auth(&state, &headers).await?;

    sqlx::query("DELETE FROM write_keys WHERE project_id = $1")
        .bind(&project_id)
        .execute(&state.pg)
        .await
        .map_err(|_| internal_error())?;

    let result = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(&project_id)
        .execute(&state.pg)
        .await
        .map_err(|_| internal_error())?;

    if result.rows_affected() == 0 {
        return Err(not_found());
    }

    Ok(Json(serde_json::json!({"status": "deleted"})))
}

pub async fn get_project_write_keys(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<WriteKeyListResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM projects WHERE id = $1")
            .bind(&project_id)
            .fetch_optional(&state.pg)
            .await
            .map_err(|_| internal_error())?;

    if exists.is_none() {
        return Err(not_found());
    }

    #[derive(sqlx::FromRow)]
    struct KeyRow {
        id: Uuid,
        key: String,
        created_at: chrono::DateTime<chrono::Utc>,
        revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let keys: Vec<KeyRow> = sqlx::query_as(
        "SELECT id, key, created_at, revoked_at FROM write_keys WHERE project_id = $1 ORDER BY created_at DESC",
    )
    .bind(&project_id)
    .fetch_all(&state.pg)
    .await
    .map_err(|_| internal_error())?;

    let write_keys = keys
        .into_iter()
        .map(|k| WriteKeyResponse {
            id: k.id,
            key: k.key,
            created_at: k.created_at.to_rfc3339(),
            revoked_at: k.revoked_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    Ok(Json(WriteKeyListResponse { write_keys }))
}

pub async fn create_write_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(project_id): Path<String>,
) -> Result<Json<WriteKeyResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    let exists: Option<(String,)> =
        sqlx::query_as("SELECT id FROM projects WHERE id = $1")
            .bind(&project_id)
            .fetch_optional(&state.pg)
            .await
            .map_err(|_| internal_error())?;

    if exists.is_none() {
        return Err(not_found());
    }

    let write_key = format!("wk_{}", Uuid::new_v4().to_string().replace('-', ""));
    let key_id = Uuid::new_v4();

    sqlx::query("INSERT INTO write_keys (id, project_id, key) VALUES ($1, $2, $3)")
        .bind(key_id)
        .bind(&project_id)
        .bind(&write_key)
        .execute(&state.pg)
        .await
        .map_err(|_| internal_error())?;

    Ok(Json(WriteKeyResponse {
        id: key_id,
        key: write_key,
        created_at: chrono::Utc::now().to_rfc3339(),
        revoked_at: None,
    }))
}

pub async fn revoke_write_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path((project_id, key_id)): Path<(String, Uuid)>,
) -> Result<Json<serde_json::Value>, AdminError> {
    admin_auth(&state, &headers).await?;

    let result = sqlx::query(
        "UPDATE write_keys SET revoked_at = now() WHERE id = $1 AND project_id = $2 AND revoked_at IS NULL",
    )
    .bind(key_id)
    .bind(&project_id)
    .execute(&state.pg)
    .await
    .map_err(|_| internal_error())?;

    if result.rows_affected() == 0 {
        return Err(not_found());
    }

    Ok(Json(serde_json::json!({"status": "revoked"})))
}

pub async fn health(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<HealthCheckResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    let pg_ok = sqlx::query("SELECT 1")
        .execute(&state.pg)
        .await
        .is_ok();

    let mut redis = state.redis.clone();
    let redis_ok = redis::cmd("PING")
        .query_async::<_, String>(&mut redis)
        .await
        .is_ok();

    let query_ok = state
        .http_client
        .get(format!("{}/health", state.query_url))
        .send()
        .await
        .ok()
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    let all_ok = pg_ok && redis_ok && query_ok;
    let status = if all_ok { "ok" } else { "degraded" };

    Ok(Json(HealthCheckResponse {
        status: status.to_string(),
        checks: serde_json::json!({
            "postgres": if pg_ok { "ok" } else { "unhealthy" },
            "redis": if redis_ok { "ok" } else { "unhealthy" },
            "query": if query_ok { "ok" } else { "unhealthy" },
        }),
    }))
}
