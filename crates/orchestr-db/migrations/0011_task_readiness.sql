PRAGMA foreign_keys = OFF;

CREATE TABLE tasks_m14 (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL CHECK (status IN ('backlog', 'ready', 'in_progress', 'review', 'approved', 'integrating', 'blocked', 'done')),
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
    priority TEXT NOT NULL DEFAULT 'normal' CHECK (priority IN ('critical', 'high', 'normal', 'low')),
    blocked_reason TEXT,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT
);

INSERT INTO tasks_m14 (
    id, project_id, title, description, status, position, created_at, updated_at,
    acceptance_criteria, implementation_notes, relevant_paths, dependency_ids,
    assigned_agent_id, branch, worktree_path, priority, blocked_reason
)
SELECT
    id, project_id, title, description,
    CASE status WHEN 'todo' THEN 'ready' ELSE status END,
    position, created_at, updated_at, acceptance_criteria, implementation_notes,
    relevant_paths, dependency_ids, assigned_agent_id, branch, worktree_path,
    'normal', NULL
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_m14 RENAME TO tasks;

CREATE INDEX idx_tasks_project_status_position ON tasks(project_id, status, position);
CREATE INDEX idx_tasks_assigned_agent ON tasks(assigned_agent_id);
CREATE UNIQUE INDEX idx_tasks_branch ON tasks(branch) WHERE branch IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_worktree_path ON tasks(worktree_path) WHERE worktree_path IS NOT NULL;

PRAGMA foreign_keys = ON;
