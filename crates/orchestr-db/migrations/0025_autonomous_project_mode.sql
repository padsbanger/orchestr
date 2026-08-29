CREATE TABLE project_autonomy (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    status TEXT NOT NULL DEFAULT 'stopped'
        CHECK (status IN ('stopped', 'running', 'paused', 'completed')),
    planning_proposal_id TEXT REFERENCES planning_proposals(id) ON DELETE SET NULL,
    reviewer_agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
    auto_schedule INTEGER NOT NULL DEFAULT 1 CHECK (auto_schedule IN (0, 1)),
    auto_review INTEGER NOT NULL DEFAULT 1 CHECK (auto_review IN (0, 1)),
    auto_integrate INTEGER NOT NULL DEFAULT 1 CHECK (auto_integrate IN (0, 1)),
    max_tasks_per_cycle INTEGER NOT NULL DEFAULT 2 CHECK (max_tasks_per_cycle BETWEEN 1 AND 20),
    max_auto_retries INTEGER NOT NULL DEFAULT 1 CHECK (max_auto_retries BETWEEN 0 AND 3),
    pause_on_failure INTEGER NOT NULL DEFAULT 1 CHECK (pause_on_failure IN (0, 1)),
    pause_on_needs_input INTEGER NOT NULL DEFAULT 1 CHECK (pause_on_needs_input IN (0, 1)),
    pause_reason TEXT,
    started_at TEXT,
    stopped_at TEXT,
    last_cycle_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE autonomy_cycles (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    trigger_kind TEXT NOT NULL CHECK (trigger_kind IN ('user', 'timer', 'event')),
    status TEXT NOT NULL DEFAULT 'running'
        CHECK (status IN ('running', 'completed', 'paused', 'failed', 'skipped')),
    scheduled_count INTEGER NOT NULL DEFAULT 0,
    review_count INTEGER NOT NULL DEFAULT 0,
    retry_count INTEGER NOT NULL DEFAULT 0,
    integration_count INTEGER NOT NULL DEFAULT 0,
    outcome TEXT,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE UNIQUE INDEX idx_autonomy_cycles_one_running
    ON autonomy_cycles(project_id) WHERE status = 'running';
CREATE INDEX idx_autonomy_cycles_project_started
    ON autonomy_cycles(project_id, started_at DESC);

CREATE TABLE autonomy_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    cycle_id TEXT REFERENCES autonomy_cycles(id) ON DELETE SET NULL,
    kind TEXT NOT NULL,
    message TEXT NOT NULL,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_autonomy_events_project_created
    ON autonomy_events(project_id, id DESC);
