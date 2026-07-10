CREATE TABLE IF NOT EXISTS replay_sessions (
    project_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    distinct_id TEXT,
    started_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_activity TIMESTAMPTZ NOT NULL DEFAULT now(),
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (project_id, session_id)
);

CREATE INDEX IF NOT EXISTS idx_replay_sessions_distinct_id ON replay_sessions(project_id, distinct_id);
CREATE INDEX IF NOT EXISTS idx_replay_sessions_status ON replay_sessions(project_id, status);
CREATE INDEX IF NOT EXISTS idx_replay_sessions_started_at ON replay_sessions(project_id, started_at DESC);
