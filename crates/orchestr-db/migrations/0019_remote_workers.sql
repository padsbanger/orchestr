CREATE TABLE remote_workers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    endpoint TEXT NOT NULL UNIQUE,
    token_environment_variable TEXT NOT NULL,
    ca_certificate_pem TEXT,
    os TEXT NOT NULL,
    architecture TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'online' CHECK (status IN ('online', 'offline')),
    protocol_version INTEGER NOT NULL,
    tools TEXT NOT NULL DEFAULT '[]',
    last_seen_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE remote_worker_projects (
    worker_id TEXT NOT NULL REFERENCES remote_workers(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    workspace_path TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (worker_id, project_id)
);

CREATE UNIQUE INDEX idx_remote_worker_projects_enabled_project
    ON remote_worker_projects(project_id) WHERE enabled = 1;
CREATE INDEX idx_remote_workers_status ON remote_workers(status, last_seen_at DESC);
