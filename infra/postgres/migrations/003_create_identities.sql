CREATE TABLE IF NOT EXISTS identities (
    project_id TEXT NOT NULL,
    anonymous_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(project_id, anonymous_id)
);

CREATE INDEX IF NOT EXISTS idx_identities_user_id ON identities(project_id, user_id);
