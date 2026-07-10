CREATE TABLE IF NOT EXISTS replay_chunks (
    project_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    window_id TEXT NOT NULL,
    chunk_seq INTEGER NOT NULL,
    s3_key TEXT NOT NULL,
    s3_bucket TEXT NOT NULL,
    byte_size INTEGER NOT NULL DEFAULT 0,
    event_count INTEGER NOT NULL DEFAULT 0,
    is_final BOOLEAN NOT NULL DEFAULT false,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, session_id, chunk_seq)
);

CREATE INDEX IF NOT EXISTS idx_replay_chunks_session ON replay_chunks(project_id, session_id, chunk_seq);
