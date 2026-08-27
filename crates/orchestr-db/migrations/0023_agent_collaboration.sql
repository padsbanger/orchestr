CREATE TABLE collaboration_entries (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES collaboration_entries(id) ON DELETE CASCADE,
    author_type TEXT NOT NULL CHECK (author_type IN ('human', 'agent', 'system')),
    author_agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
    author_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
    kind TEXT NOT NULL CHECK (kind IN ('comment', 'request', 'blocker', 'interface_change', 'escalation')),
    message TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'resolved')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT
);

CREATE TABLE collaboration_entry_references (
    entry_id TEXT NOT NULL REFERENCES collaboration_entries(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (entry_id, task_id)
);

CREATE INDEX idx_collaboration_project_created
    ON collaboration_entries(project_id, created_at DESC);
CREATE INDEX idx_collaboration_task_created
    ON collaboration_entries(task_id, created_at DESC);
CREATE INDEX idx_collaboration_parent_created
    ON collaboration_entries(parent_id, created_at ASC);
CREATE INDEX idx_collaboration_references_task
    ON collaboration_entry_references(task_id, entry_id);
