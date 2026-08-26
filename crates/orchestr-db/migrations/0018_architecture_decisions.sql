CREATE TABLE architecture_decisions (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    decision_number INTEGER NOT NULL,
    title TEXT NOT NULL,
    context TEXT NOT NULL,
    decision TEXT NOT NULL,
    consequences TEXT,
    status TEXT NOT NULL DEFAULT 'proposed'
        CHECK (status IN ('proposed', 'accepted', 'superseded', 'rejected')),
    supersedes_decision_id TEXT REFERENCES architecture_decisions(id) ON DELETE SET NULL,
    relevant_paths TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    decided_at TEXT,
    UNIQUE (project_id, decision_number),
    CHECK (supersedes_decision_id IS NULL OR supersedes_decision_id <> id)
);

CREATE TABLE architecture_decision_tasks (
    decision_id TEXT NOT NULL REFERENCES architecture_decisions(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (decision_id, task_id)
);

CREATE INDEX idx_architecture_decisions_project_status
    ON architecture_decisions(project_id, status, decision_number DESC);
CREATE INDEX idx_architecture_decision_tasks_task
    ON architecture_decision_tasks(task_id);
