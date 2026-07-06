use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::SharedState;

#[derive(Deserialize)]
pub struct SetupRequest {
    email: String,
    password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    token: String,
    admin_id: Uuid,
    email: String,
}

#[derive(Serialize)]
pub struct MeResponse {
    admin_id: Uuid,
    email: String,
    created_at: String,
    last_login_at: Option<String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

type AuthError = (StatusCode, Json<ErrorResponse>);

fn internal_error() -> AuthError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: "internal_error".into(),
        }),
    )
}

async fn hash_password(password: &str) -> Result<String, AuthError> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|_| internal_error())
}

fn verify_password(password: &str, hash: &str) -> Result<(), AuthError> {
    let parsed = PasswordHash::new(hash).map_err(|_| internal_error())?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "invalid_credentials".into(),
                }),
            )
        })
}

async fn create_session(
    state: &SharedState,
    admin_id: Uuid,
    email: &str,
) -> Result<String, AuthError> {
    let token = Uuid::new_v4().to_string();
    let session_key = format!("session:{}", token);
    let session_data = serde_json::json!({
        "admin_id": admin_id.to_string(),
        "email": email,
    });

    let mut redis = state.redis.clone();
    redis::cmd("SETEX")
        .arg(&session_key)
        .arg(state.session_ttl_seconds as i64)
        .arg(session_data.to_string())
        .query_async::<_, ()>(&mut redis)
        .await
        .map_err(|_| internal_error())?;

    Ok(token)
}

fn extract_session_token(
    headers: &HeaderMap,
) -> Result<String, AuthError> {
    let cookie = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "unauthorized".into(),
                }),
            )
        })?;
    Ok(cookie)
}

pub async fn get_session(
    state: &SharedState,
    headers: &HeaderMap,
) -> Result<(Uuid, String), AuthError> {
    let token = extract_session_token(headers)?;
    let session_key = format!("session:{}", token);

    let mut redis = state.redis.clone();
    let data: Option<String> = redis::cmd("GET")
        .arg(&session_key)
        .query_async(&mut redis)
        .await
        .map_err(|_| internal_error())?;

    match data {
        Some(json) => {
            let parsed: serde_json::Value =
                serde_json::from_str(&json).map_err(|_| internal_error())?;
            let admin_id = parsed["admin_id"]
                .as_str()
                .and_then(|s| s.parse::<Uuid>().ok())
                .ok_or_else(internal_error)?;
            let email = parsed["email"].as_str().unwrap_or("").to_string();
            Ok((admin_id, email))
        }
        None => Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "session_expired".into(),
            }),
        )),
    }
}

pub async fn setup(
    State(state): State<SharedState>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<AuthResponse>, AuthError> {
    if req.email.is_empty() || req.password.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_failed".into(),
            }),
        ));
    }

    if req.password.len() < 8 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "validation_failed".into(),
            }),
        ));
    }

    let existing: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM admin_users LIMIT 1",
    )
    .fetch_optional(&state.pg)
    .await
    .map_err(|_| internal_error())?;

    if existing.is_some() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "already_setup".into(),
            }),
        ));
    }

    let password_hash = hash_password(&req.password).await?;
    let admin_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO admin_users (id, email, password_hash) VALUES ($1, $2, $3)",
    )
    .bind(admin_id)
    .bind(&req.email)
    .bind(&password_hash)
    .execute(&state.pg)
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "failed to create admin user");
        internal_error()
    })?;

    let token = create_session(&state, admin_id, &req.email).await?;

    Ok(Json(AuthResponse {
        token,
        admin_id,
        email: req.email,
    }))
}

pub async fn login(
    State(state): State<SharedState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AuthError> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, password_hash FROM admin_users WHERE email = $1",
    )
    .bind(&req.email)
    .fetch_optional(&state.pg)
    .await
    .map_err(|_| internal_error())?;

    let (admin_id, password_hash) = row.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "invalid_credentials".into(),
            }),
        )
    })?;

    verify_password(&req.password, &password_hash)?;

    sqlx::query("UPDATE admin_users SET last_login_at = now() WHERE id = $1")
        .bind(admin_id)
        .execute(&state.pg)
        .await
        .map_err(|_| internal_error())?;

    let token = create_session(&state, admin_id, &req.email).await?;

    Ok(Json(AuthResponse {
        token,
        admin_id,
        email: req.email,
    }))
}

pub async fn logout(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, AuthError> {
    let token = extract_session_token(&headers)?;
    let session_key = format!("session:{}", token);

    let mut redis = state.redis.clone();
    redis::cmd("DEL")
        .arg(&session_key)
        .query_async::<_, ()>(&mut redis)
        .await
        .map_err(|_| internal_error())?;

    Ok(Json(serde_json::json!({"status": "ok"})))
}

pub async fn me(
    State(state): State<SharedState>,
    headers: HeaderMap,
) -> Result<Json<MeResponse>, AuthError> {
    let (admin_id, _) = get_session(&state, &headers).await?;

    let row: Option<(String, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT email, last_login_at FROM admin_users WHERE id = $1",
        )
        .bind(admin_id)
        .fetch_optional(&state.pg)
        .await
        .map_err(|_| internal_error())?;

    let (email, last_login_at) = row.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "admin_not_found".into(),
            }),
        )
    })?;

    Ok(Json(MeResponse {
        admin_id,
        email,
        created_at: chrono::Utc::now().to_rfc3339(),
        last_login_at: last_login_at.map(|dt| dt.to_rfc3339()),
    }))
}
