CREATE TABLE IF NOT EXISTS identities (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id TEXT NOT NULL,
    anonymous_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(project_id, anonymous_id)
);

CREATE INDEX IF NOT EXISTS idx_identities_project_anonymous ON identities(project_id, anonymous_id);
CREATE INDEX IF NOT EXISTS idx_identities_user_id ON identities(project_id, user_id);
