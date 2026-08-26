CREATE TABLE planning_proposals (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
    goal TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('generating', 'proposed', 'approved', 'rejected', 'failed', 'cancelled')),
    plan_json TEXT,
    raw_output TEXT NOT NULL DEFAULT '',
    error TEXT,
    milestone_id TEXT REFERENCES milestones(id) ON DELETE SET NULL,
    epic_id TEXT REFERENCES epics(id) ON DELETE SET NULL,
    task_ids TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TEXT,
    decided_at TEXT
);

CREATE INDEX idx_planning_proposals_project_created
    ON planning_proposals(project_id, created_at DESC);
