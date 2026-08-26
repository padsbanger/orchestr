PRAGMA foreign_keys = OFF;

CREATE TABLE tasks_m19 (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL CHECK (status IN ('backlog', 'ready', 'in_progress', 'needs_input', 'review', 'approved', 'integrating', 'blocked', 'done')),
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
    readiness_blocked INTEGER NOT NULL DEFAULT 0 CHECK (readiness_blocked IN (0, 1)),
    milestone_id TEXT REFERENCES milestones(id) ON DELETE SET NULL,
    epic_id TEXT REFERENCES epics(id) ON DELETE SET NULL,
    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE RESTRICT
);

INSERT INTO tasks_m19 (
    id, project_id, title, description, status, position, created_at, updated_at,
    acceptance_criteria, implementation_notes, relevant_paths, dependency_ids,
    assigned_agent_id, branch, worktree_path, priority, blocked_reason,
    readiness_blocked, milestone_id, epic_id
)
SELECT
    id, project_id, title, description, status, position, created_at, updated_at,
    acceptance_criteria, implementation_notes, relevant_paths, dependency_ids,
    assigned_agent_id, branch, worktree_path, priority, blocked_reason,
    readiness_blocked, milestone_id, epic_id
FROM tasks;

DROP TABLE tasks;
ALTER TABLE tasks_m19 RENAME TO tasks;

CREATE INDEX idx_tasks_project_status_position ON tasks(project_id, status, position);
CREATE INDEX idx_tasks_assigned_agent ON tasks(assigned_agent_id);
CREATE UNIQUE INDEX idx_tasks_branch ON tasks(branch) WHERE branch IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_worktree_path ON tasks(worktree_path) WHERE worktree_path IS NOT NULL;
CREATE INDEX idx_tasks_milestone ON tasks(milestone_id);
CREATE INDEX idx_tasks_epic ON tasks(epic_id);

CREATE TRIGGER prevent_active_task_edit
BEFORE UPDATE OF title, description, acceptance_criteria, implementation_notes,
    relevant_paths, dependency_ids, assigned_agent_id, priority, milestone_id, epic_id ON tasks
WHEN EXISTS (
    SELECT 1 FROM runs WHERE task_id = OLD.id AND status IN ('queued', 'running')
)
BEGIN
    SELECT RAISE(ABORT, 'Cancel or finish the queued run before editing this task.');
END;

CREATE TRIGGER prevent_active_task_move
BEFORE UPDATE OF status ON tasks
WHEN EXISTS (
    SELECT 1 FROM runs WHERE task_id = OLD.id AND status IN ('queued', 'running')
)
AND NOT (
    (OLD.status = 'ready' AND NEW.status = 'in_progress'
        AND EXISTS (SELECT 1 FROM runs WHERE task_id = OLD.id AND status = 'running'))
    OR (OLD.status = 'in_progress' AND NEW.status = 'needs_input')
)
BEGIN
    SELECT RAISE(ABORT, 'Cancel or finish the queued run before moving this task.');
END;

PRAGMA foreign_keys = ON;

CREATE TABLE task_input_requests (
    id TEXT PRIMARY KEY NOT NULL,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    requesting_run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
    requesting_agent_id TEXT REFERENCES agents(id) ON DELETE SET NULL,
    question TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'answered')),
    answer TEXT,
    requested_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    answered_at TEXT
);

CREATE UNIQUE INDEX idx_task_input_requests_one_open
    ON task_input_requests(task_id) WHERE status = 'open';
CREATE INDEX idx_task_input_requests_task_requested
    ON task_input_requests(task_id, requested_at DESC);

CREATE TABLE project_blockers (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    affects_all_tasks INTEGER NOT NULL DEFAULT 0 CHECK (affects_all_tasks IN (0, 1)),
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'resolved')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    resolved_at TEXT
);

CREATE TABLE project_blocker_tasks (
    blocker_id TEXT NOT NULL REFERENCES project_blockers(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    PRIMARY KEY (blocker_id, task_id)
);

CREATE INDEX idx_project_blockers_project_status
    ON project_blockers(project_id, status, created_at DESC);
CREATE INDEX idx_project_blocker_tasks_task ON project_blocker_tasks(task_id);
