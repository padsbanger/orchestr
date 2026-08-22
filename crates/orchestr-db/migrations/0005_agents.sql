CREATE TABLE agents (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    provider TEXT NOT NULL,
    role TEXT NOT NULL,
    model TEXT,
    system_prompt TEXT,
    skills TEXT NOT NULL DEFAULT '[]',
    max_concurrent_tasks INTEGER NOT NULL DEFAULT 1 CHECK (max_concurrent_tasks > 0),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE tasks ADD COLUMN assigned_agent_id TEXT;

CREATE INDEX idx_tasks_assigned_agent ON tasks(assigned_agent_id);
