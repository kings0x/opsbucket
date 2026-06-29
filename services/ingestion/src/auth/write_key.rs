pub async fn validate_write_key(
    _key: &str,
    _redis: &redis::Client,
    _pg: &sqlx::PgPool,
) -> Option<String> {
    None
}
