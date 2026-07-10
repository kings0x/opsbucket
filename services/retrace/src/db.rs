use anyhow::Result;
use async_trait::async_trait;
use sqlx::PgPool;

#[async_trait]
pub trait ReplayStore: Send + Sync {
    async fn upsert_session(
        &self,
        project_id: &str,
        session_id: &str,
        distinct_id: Option<&str>,
        is_final: bool,
        chunk_count_delta: i32,
    ) -> Result<()>;

    async fn insert_chunk(
        &self,
        project_id: &str,
        session_id: &str,
        window_id: &str,
        chunk_seq: u32,
        s3_key: &str,
        s3_bucket: &str,
        byte_size: i32,
        event_count: i32,
        is_final: bool,
    ) -> Result<()>;
}

pub struct PostgresStore {
    pool: PgPool,
}

impl PostgresStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReplayStore for PostgresStore {
    async fn upsert_session(
        &self,
        project_id: &str,
        session_id: &str,
        distinct_id: Option<&str>,
        is_final: bool,
        chunk_count_delta: i32,
    ) -> Result<()> {
        let status = if is_final { "completed" } else { "active" };
        sqlx::query(
            r#"
            INSERT INTO replay_sessions (project_id, session_id, distinct_id, started_at, last_activity, chunk_count, status)
            VALUES ($1, $2, $3, now(), now(), $4, $5)
            ON CONFLICT (project_id, session_id)
            DO UPDATE SET
                last_activity = now(),
                chunk_count = replay_sessions.chunk_count + $4,
                status = CASE
                    WHEN $5 = 'completed' THEN 'completed'
                    ELSE replay_sessions.status
                END,
                updated_at = now()
            "#,
        )
        .bind(project_id)
        .bind(session_id)
        .bind(distinct_id)
        .bind(chunk_count_delta)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn insert_chunk(
        &self,
        project_id: &str,
        session_id: &str,
        window_id: &str,
        chunk_seq: u32,
        s3_key: &str,
        s3_bucket: &str,
        byte_size: i32,
        event_count: i32,
        is_final: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO replay_chunks (project_id, session_id, window_id, chunk_seq, s3_key, s3_bucket, byte_size, event_count, is_final, received_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())
            ON CONFLICT (project_id, session_id, chunk_seq) DO NOTHING
            "#,
        )
        .bind(project_id)
        .bind(session_id)
        .bind(window_id)
        .bind(chunk_seq as i32)
        .bind(s3_key)
        .bind(s3_bucket)
        .bind(byte_size)
        .bind(event_count)
        .bind(is_final)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[cfg(test)]
pub struct MockReplayStore {
    pub sessions: std::sync::Mutex<Vec<(String, String, Option<String>, bool, i32)>>,
    pub chunks: std::sync::Mutex<Vec<(String, String, String, u32, String, String, i32, i32, bool)>>,
    pub should_fail: bool,
}

#[cfg(test)]
impl MockReplayStore {
    pub fn new() -> Self {
        Self {
            sessions: std::sync::Mutex::new(Vec::new()),
            chunks: std::sync::Mutex::new(Vec::new()),
            should_fail: false,
        }
    }

    pub fn with_failure() -> Self {
        Self {
            sessions: std::sync::Mutex::new(Vec::new()),
            chunks: std::sync::Mutex::new(Vec::new()),
            should_fail: true,
        }
    }
}

#[cfg(test)]
#[async_trait]
impl ReplayStore for MockReplayStore {
    async fn upsert_session(
        &self,
        project_id: &str,
        session_id: &str,
        distinct_id: Option<&str>,
        is_final: bool,
        chunk_count_delta: i32,
    ) -> Result<()> {
        if self.should_fail {
            anyhow::bail!("postgres upsert failed (mock)");
        }
        self.sessions
            .lock()
            .unwrap()
            .push((
                project_id.to_string(),
                session_id.to_string(),
                distinct_id.map(|s| s.to_string()),
                is_final,
                chunk_count_delta,
            ));
        Ok(())
    }

    async fn insert_chunk(
        &self,
        project_id: &str,
        session_id: &str,
        window_id: &str,
        chunk_seq: u32,
        s3_key: &str,
        s3_bucket: &str,
        byte_size: i32,
        event_count: i32,
        is_final: bool,
    ) -> Result<()> {
        if self.should_fail {
            anyhow::bail!("postgres insert failed (mock)");
        }
        self.chunks
            .lock()
            .unwrap()
            .push((
                project_id.to_string(),
                session_id.to_string(),
                window_id.to_string(),
                chunk_seq,
                s3_key.to_string(),
                s3_bucket.to_string(),
                byte_size,
                event_count,
                is_final,
            ));
        Ok(())
    }
}
