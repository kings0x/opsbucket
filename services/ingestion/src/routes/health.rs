use axum::{http::StatusCode, response::IntoResponse};

pub async fn get_health() -> impl IntoResponse {
    StatusCode::OK
}
