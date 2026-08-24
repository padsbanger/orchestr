CREATE TABLE agent_reviews (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    agent_id TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('running', 'completed', 'failed', 'cancelled')),
    decision TEXT CHECK (decision IN ('approve', 'request_changes')),
    notes TEXT,
    raw_output TEXT NOT NULL DEFAULT '',
    error TEXT,
    started_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT
);

CREATE INDEX idx_agent_reviews_task_started ON agent_reviews(task_id, started_at DESC);
