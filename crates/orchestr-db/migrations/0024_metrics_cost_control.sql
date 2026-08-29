CREATE TABLE task_status_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    from_status TEXT,
    to_status TEXT NOT NULL,
    changed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO task_status_history (task_id, from_status, to_status, changed_at)
SELECT id, NULL, status, updated_at FROM tasks;

CREATE INDEX idx_task_status_history_task_changed
    ON task_status_history(task_id, changed_at, id);
CREATE INDEX idx_task_status_history_status_changed
    ON task_status_history(to_status, changed_at);

CREATE TRIGGER record_task_status_change
AFTER UPDATE OF status ON tasks
WHEN OLD.status <> NEW.status
BEGIN
    INSERT INTO task_status_history (task_id, from_status, to_status)
    VALUES (NEW.id, OLD.status, NEW.status);
END;

CREATE TABLE project_cost_controls (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    monthly_budget_micros INTEGER NOT NULL DEFAULT 0 CHECK (monthly_budget_micros >= 0),
    warning_threshold_percent INTEGER NOT NULL DEFAULT 80 CHECK (warning_threshold_percent BETWEEN 1 AND 100),
    block_new_runs INTEGER NOT NULL DEFAULT 0 CHECK (block_new_runs IN (0, 1)),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE provider_model_pricing (
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_micros_per_million INTEGER NOT NULL CHECK (input_micros_per_million >= 0),
    cached_input_micros_per_million INTEGER NOT NULL CHECK (cached_input_micros_per_million >= 0),
    output_micros_per_million INTEGER NOT NULL CHECK (output_micros_per_million >= 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (project_id, provider, model)
);

CREATE TABLE run_usage (
    run_id TEXT PRIMARY KEY REFERENCES runs(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
    cached_input_tokens INTEGER NOT NULL DEFAULT 0 CHECK (cached_input_tokens >= 0),
    output_tokens INTEGER NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
    estimated_cost_micros INTEGER NOT NULL DEFAULT 0 CHECK (estimated_cost_micros >= 0),
    priced INTEGER NOT NULL DEFAULT 0 CHECK (priced IN (0, 1)),
    recorded_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_run_usage_provider_model ON run_usage(provider, model);
