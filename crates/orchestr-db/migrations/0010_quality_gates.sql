CREATE TABLE validation_commands (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    stage TEXT NOT NULL CHECK (stage IN ('implementation', 'integration')),
    name TEXT NOT NULL,
    program TEXT NOT NULL,
    arguments TEXT NOT NULL DEFAULT '[]',
    position INTEGER NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_validation_commands_project_stage_position
    ON validation_commands(project_id, stage, position);

CREATE TABLE validation_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    integration_attempt_id TEXT REFERENCES integration_attempts(id) ON DELETE SET NULL,
    stage TEXT NOT NULL CHECK (stage IN ('implementation', 'integration')),
    status TEXT NOT NULL CHECK (status IN ('running', 'passed', 'failed', 'cancelled')),
    error TEXT,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE INDEX idx_validation_attempts_project_stage_started
    ON validation_attempts(project_id, stage, started_at DESC);
CREATE INDEX idx_validation_attempts_task_started
    ON validation_attempts(task_id, started_at DESC);

CREATE TABLE validation_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    validation_attempt_id TEXT NOT NULL REFERENCES validation_attempts(id) ON DELETE CASCADE,
    validation_command_id TEXT REFERENCES validation_commands(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    message TEXT NOT NULL,
    stream TEXT,
    exit_code INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_validation_events_attempt_id ON validation_events(validation_attempt_id, id);

CREATE TABLE project_health (
    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'unknown' CHECK (status IN ('unknown', 'healthy', 'degraded', 'broken')),
    last_validation_attempt_id TEXT REFERENCES validation_attempts(id) ON DELETE SET NULL,
    last_successful_validation_at TEXT,
    last_integration_at TEXT,
    failing_gate TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO project_health (project_id) SELECT id FROM projects;
