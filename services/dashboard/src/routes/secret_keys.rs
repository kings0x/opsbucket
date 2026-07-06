use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::auth;
use crate::SharedState;

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

type AdminError = (StatusCode, Json<ErrorResponse>);

#[derive(Serialize)]
pub struct SecretKeyResponse {
    id: Uuid,
    name: String,
    key: Option<String>,
    project_id: Option<String>,
    created_at: String,
    revoked_at: Option<String>,
}

#[derive(Serialize)]
pub struct SecretKeyListResponse {
    secret_keys: Vec<SecretKeyResponse>,
}

#[derive(Deserialize)]
pub struct CreateSecretKeyRequest {
    name: String,
    project_id: Option<String>,
}

#[derive(Deserialize)]
pub struct ListSecretKeysQuery {
    project_id: Option<String>,
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

pub async fn list_secret_keys(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Query(query): Query<ListSecretKeysQuery>,
) -> Result<Json<SecretKeyListResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct KeyRow {
        id: Uuid,
        name: String,
        project_id: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
        revoked_at: Option<chrono::DateTime<chrono::Utc>>,
    }

    let rows: Vec<KeyRow> = match &query.project_id {
        Some(pid) if !pid.is_empty() => sqlx::query_as(
            "SELECT id, name, project_id, created_at, revoked_at FROM secret_keys WHERE project_id = $1 ORDER BY created_at DESC",
        )
        .bind(pid)
        .fetch_all(&state.pg)
        .await
        .map_err(|_| internal_error())?,
        _ => sqlx::query_as(
            "SELECT id, name, project_id, created_at, revoked_at FROM secret_keys ORDER BY created_at DESC",
        )
        .fetch_all(&state.pg)
        .await
        .map_err(|_| internal_error())?,
    };

    let secret_keys = rows
        .into_iter()
        .map(|k| SecretKeyResponse {
            id: k.id,
            name: k.name,
            key: None,
            project_id: k.project_id,
            created_at: k.created_at.to_rfc3339(),
            revoked_at: k.revoked_at.map(|dt| dt.to_rfc3339()),
        })
        .collect();

    Ok(Json(SecretKeyListResponse { secret_keys }))
}

pub async fn create_secret_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Json(req): Json<CreateSecretKeyRequest>,
) -> Result<Json<SecretKeyResponse>, AdminError> {
    admin_auth(&state, &headers).await?;

    if req.name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_failed".into(),
            }),
        ));
    }

    let key = format!("sk_{}", Uuid::new_v4().to_string().replace('-', ""));
    let key_id = Uuid::new_v4();
    let project_id = req.project_id.unwrap_or_default();

    sqlx::query("INSERT INTO secret_keys (id, name, key, project_id) VALUES ($1, $2, $3, $4)")
        .bind(key_id)
        .bind(&req.name)
        .bind(&key)
        .bind(&project_id)
        .execute(&state.pg)
        .await
        .map_err(|_| internal_error())?;

    Ok(Json(SecretKeyResponse {
        id: key_id,
        name: req.name,
        key: Some(key),
        project_id: Some(project_id),
        created_at: chrono::Utc::now().to_rfc3339(),
        revoked_at: None,
    }))
}

pub async fn revoke_secret_key(
    State(state): State<SharedState>,
    headers: HeaderMap,
    Path(key_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AdminError> {
    admin_auth(&state, &headers).await?;

    let result = sqlx::query(
        "UPDATE secret_keys SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(key_id)
    .execute(&state.pg)
    .await
    .map_err(|_| internal_error())?;

    if result.rows_affected() == 0 {
        return Err(not_found());
    }

    Ok(Json(serde_json::json!({"status": "revoked"})))
}
