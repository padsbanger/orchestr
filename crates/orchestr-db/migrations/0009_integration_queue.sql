PRAGMA foreign_keys = OFF;

CREATE TABLE tasks_m12 (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL CHECK (status IN ('backlog', 'todo', 'in_progress', 'review', 'approved', 'integrating', 'blocked', 'done')),
    position INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    acceptance_criteria TEXT NOT NULL DEFAULT '[]',
    implementation_notes TEXT,
    relevant_paths TEXT NOT NULL DEFAULT '[]',
    dependency_ids TEXT NOT NULL DEFAULT '[]',
    assigned_agent_id TEXT,
    branch TEXT,
    worktree_path TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT
);

INSERT INTO tasks_m12 (
    id, project_id, title, description, status, position, created_at, updated_at,
    acceptance_criteria, implementation_notes, relevant_paths, dependency_ids,
    assigned_agent_id, branch, worktree_path
)
SELECT
    id, project_id, title, description, status, position, created_at, updated_at,
    acceptance_criteria, implementation_notes, relevant_paths, dependency_ids,
    assigned_agent_id, branch, worktree_path
FROM tasks;

-- M11 approval previously moved reviewed task branches directly to Done. A task
-- that still owns a worktree has not been integrated into the primary branch,
-- so return it to Review for explicit approval and queued integration.
UPDATE tasks_m12 SET status = 'review' WHERE status = 'done' AND worktree_path IS NOT NULL;

DROP TABLE tasks;
ALTER TABLE tasks_m12 RENAME TO tasks;

CREATE INDEX idx_tasks_project_status_position ON tasks(project_id, status, position);
CREATE INDEX idx_tasks_assigned_agent ON tasks(assigned_agent_id);
CREATE UNIQUE INDEX idx_tasks_branch ON tasks(branch) WHERE branch IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_worktree_path ON tasks(worktree_path) WHERE worktree_path IS NOT NULL;

CREATE TABLE integration_attempts (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    source_branch TEXT NOT NULL,
    target_branch TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'integrating', 'conflict', 'merged', 'failed')),
    queue_position INTEGER NOT NULL,
    merge_commit TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    started_at TEXT,
    completed_at TEXT,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_integration_attempts_task_created ON integration_attempts(task_id, created_at DESC);
CREATE INDEX idx_integration_attempts_queue ON integration_attempts(status, queue_position);

CREATE TABLE project_integration_locks (
    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    attempt_id TEXT NOT NULL UNIQUE REFERENCES integration_attempts(id) ON DELETE CASCADE,
    acquired_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

PRAGMA foreign_keys = ON;
