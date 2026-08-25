CREATE TABLE run_recoveries (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    source_run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    replacement_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
    agent_id TEXT,
    action TEXT NOT NULL CHECK (action IN ('resume', 'restart_clean', 'reassign', 'abandon', 'escalate')),
    note TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_run_recoveries_task_created ON run_recoveries(task_id, created_at DESC);

CREATE TABLE revert_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    original_task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    integration_attempt_id TEXT NOT NULL REFERENCES integration_attempts(id) ON DELETE CASCADE,
    original_commit TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('running', 'reverted', 'validation_failed', 'failed')),
    revert_commit TEXT,
    repair_task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    error TEXT,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE INDEX idx_revert_attempts_project_started ON revert_attempts(project_id, started_at DESC);
CREATE INDEX idx_revert_attempts_integration ON revert_attempts(integration_attempt_id, started_at DESC);
