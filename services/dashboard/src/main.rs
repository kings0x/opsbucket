use opsbucket_dashboard::config::Config;
use opsbucket_dashboard::server::Server;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::load();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(&config.rust_log))
        .init();

    let server = Server::new(config);
    server.run().await
}

#[cfg(test)]
mod tests {
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};
    use redis::aio::ConnectionManager;
    use redis::Client as RedisClient;
    use sqlx::postgres::PgPoolOptions;
    use sqlx::PgPool;
    use uuid::Uuid;

    const PG_URL: &str = "postgres://opsbucket:opsbucket@127.0.0.1:5432/opsbucket";
    const REDIS_URL: &str = "redis://127.0.0.1:6379";

    async fn setup_db() -> (PgPool, ConnectionManager) {
        let pg = PgPoolOptions::new()
            .max_connections(5)
            .connect(PG_URL)
            .await
            .unwrap();
        sqlx::query("DELETE FROM admin_users")
            .execute(&pg)
            .await
            .unwrap();
        sqlx::query("DELETE FROM write_keys")
            .execute(&pg)
            .await
            .unwrap();
        sqlx::query("DELETE FROM projects")
            .execute(&pg)
            .await
            .unwrap();

        let client = RedisClient::open(REDIS_URL).unwrap();
        let redis = ConnectionManager::new(client).await.unwrap();
        let mut test = redis.clone();
        redis::cmd("FLUSHDB")
            .query_async::<_, ()>(&mut test)
            .await
            .unwrap();

        (pg, redis)
    }

    fn make_admin() -> (Uuid, String) {
        let id = Uuid::new_v4();
        let salt = SaltString::generate(&mut rand::rngs::OsRng);
        let hash = Argon2::default()
            .hash_password(b"testpassword123", &salt)
            .unwrap()
            .to_string();
        (id, hash)
    }

    #[tokio::test]
    async fn setup_creates_first_admin() {
        let (pg, _redis) = setup_db().await;

        let admin_id = Uuid::new_v4();
        let salt = SaltString::generate(&mut rand::rngs::OsRng);
        let hash = Argon2::default()
            .hash_password(b"testpassword123", &salt)
            .unwrap()
            .to_string();

        sqlx::query("INSERT INTO admin_users (id, email, password_hash) VALUES ($1, $2, $3)")
            .bind(admin_id)
            .bind("admin@test.com")
            .bind(&hash)
            .execute(&pg)
            .await
            .unwrap();

        let row: (String,) = sqlx::query_as("SELECT email FROM admin_users WHERE id = $1")
            .bind(admin_id)
            .fetch_one(&pg)
            .await
            .unwrap();

        assert_eq!(row.0, "admin@test.com");
    }

    #[tokio::test]
    async fn login_validates_password() {
        let (pg, _redis) = setup_db().await;
        let (admin_id, hash) = make_admin();

        sqlx::query("INSERT INTO admin_users (id, email, password_hash) VALUES ($1, $2, $3)")
            .bind(admin_id)
            .bind("admin@test.com")
            .bind(&hash)
            .execute(&pg)
            .await
            .unwrap();

        let row: Option<(String,)> =
            sqlx::query_as("SELECT password_hash FROM admin_users WHERE email = $1")
                .bind("admin@test.com")
                .fetch_optional(&pg)
                .await
                .unwrap();

        assert!(row.is_some());
    }

    #[tokio::test]
    async fn no_admin_returns_none() {
        let (pg, _redis) = setup_db().await;

        let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM admin_users LIMIT 1")
            .fetch_optional(&pg)
            .await
            .unwrap();

        assert!(row.is_none());
    }

    #[tokio::test]
    async fn create_and_list_projects() {
        let (pg, _redis) = setup_db().await;

        let project_id = format!("proj_{}", Uuid::new_v4().to_string().replace('-', ""));
        sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2)")
            .bind(&project_id)
            .bind("Test Project")
            .execute(&pg)
            .await
            .unwrap();

        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT id, name FROM projects ORDER BY created_at DESC")
                .fetch_all(&pg)
                .await
                .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, project_id);
        assert_eq!(rows[0].1, "Test Project");
    }

    #[tokio::test]
    async fn create_write_key_for_project() {
        let (pg, _redis) = setup_db().await;

        let project_id = format!("proj_{}", Uuid::new_v4().to_string().replace('-', ""));
        sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2)")
            .bind(&project_id)
            .bind("Test Project")
            .execute(&pg)
            .await
            .unwrap();

        let key = format!("wk_{}", Uuid::new_v4().to_string().replace('-', ""));

        sqlx::query("INSERT INTO write_keys (project_id, key) VALUES ($1, $2)")
            .bind(&project_id)
            .bind(&key)
            .execute(&pg)
            .await
            .unwrap();

        let rows: Vec<(String, Option<chrono::DateTime<chrono::Utc>>)> =
            sqlx::query_as("SELECT key, revoked_at FROM write_keys WHERE project_id = $1")
                .bind(&project_id)
                .fetch_all(&pg)
                .await
                .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].0, key);
        assert!(rows[0].1.is_none());
    }

    #[tokio::test]
    async fn revoke_write_key() {
        let (pg, _redis) = setup_db().await;

        let project_id = format!("proj_{}", Uuid::new_v4().to_string().replace('-', ""));
        sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2)")
            .bind(&project_id)
            .bind("Test Project")
            .execute(&pg)
            .await
            .unwrap();

        let key_id = Uuid::new_v4();
        let key = format!("wk_{}", Uuid::new_v4().to_string().replace('-', ""));

        sqlx::query("INSERT INTO write_keys (id, project_id, key) VALUES ($1, $2, $3)")
            .bind(key_id)
            .bind(&project_id)
            .bind(&key)
            .execute(&pg)
            .await
            .unwrap();

        sqlx::query(
            "UPDATE write_keys SET revoked_at = now() WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(key_id)
        .execute(&pg)
        .await
        .unwrap();

        let row: (Option<chrono::DateTime<chrono::Utc>>,) =
            sqlx::query_as("SELECT revoked_at FROM write_keys WHERE id = $1")
                .bind(key_id)
                .fetch_one(&pg)
                .await
                .unwrap();

        assert!(row.0.is_some());
    }

    #[tokio::test]
    async fn delete_project_cascades() {
        let (pg, _redis) = setup_db().await;

        let project_id = format!("proj_{}", Uuid::new_v4().to_string().replace('-', ""));
        sqlx::query("INSERT INTO projects (id, name) VALUES ($1, $2)")
            .bind(&project_id)
            .bind("Test Project")
            .execute(&pg)
            .await
            .unwrap();

        let key = format!("wk_{}", Uuid::new_v4().to_string().replace('-', ""));
        sqlx::query("INSERT INTO write_keys (project_id, key) VALUES ($1, $2)")
            .bind(&project_id)
            .bind(&key)
            .execute(&pg)
            .await
            .unwrap();

        sqlx::query("DELETE FROM write_keys WHERE project_id = $1")
            .bind(&project_id)
            .execute(&pg)
            .await
            .unwrap();

        sqlx::query("DELETE FROM projects WHERE id = $1")
            .bind(&project_id)
            .execute(&pg)
            .await
            .unwrap();

        let rows: Vec<(String,)> = sqlx::query_as("SELECT id FROM projects")
            .fetch_all(&pg)
            .await
            .unwrap();
        assert_eq!(rows.len(), 0);
    }

    #[tokio::test]
    async fn session_create_and_read() {
        let (_pg, redis) = setup_db().await;

        let admin_id = Uuid::new_v4();
        let token = Uuid::new_v4().to_string();
        let session_key = format!("session:{}", token);
        let session_data = serde_json::json!({
            "admin_id": admin_id.to_string(),
            "email": "admin@test.com",
        });

        let mut r = redis.clone();
        redis::cmd("SETEX")
            .arg(&session_key)
            .arg(3600i64)
            .arg(session_data.to_string())
            .query_async::<_, ()>(&mut r)
            .await
            .unwrap();

        let mut r = redis.clone();
        let result: Option<String> = redis::cmd("GET")
            .arg(&session_key)
            .query_async(&mut r)
            .await
            .unwrap();

        assert!(result.is_some());

        let parsed: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
        assert_eq!(parsed["email"], "admin@test.com");
    }

    #[tokio::test]
    async fn redis_flush_clears_sessions() {
        let (_pg, redis) = setup_db().await;

        let token = Uuid::new_v4().to_string();
        let session_key = format!("session:{}", token);

        let mut r = redis.clone();
        redis::cmd("SETEX")
            .arg(&session_key)
            .arg(3600i64)
            .arg(r#"{"admin_id":"abc","email":"admin@test.com"}"#)
            .query_async::<_, ()>(&mut r)
            .await
            .unwrap();

        let mut r = redis.clone();
        redis::cmd("DEL")
            .arg(&session_key)
            .query_async::<_, ()>(&mut r)
            .await
            .unwrap();

        let mut r = redis.clone();
        let result: Option<String> = redis::cmd("GET")
            .arg(&session_key)
            .query_async(&mut r)
            .await
            .unwrap();

        assert!(result.is_none());
    }
}
