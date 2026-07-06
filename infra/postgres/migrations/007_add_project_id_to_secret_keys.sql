ALTER TABLE secret_keys ADD COLUMN project_id TEXT NOT NULL DEFAULT '';
CREATE INDEX IF NOT EXISTS idx_secret_keys_project_id ON secret_keys(project_id);
