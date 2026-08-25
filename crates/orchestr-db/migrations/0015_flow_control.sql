CREATE TABLE project_flow_limits (
    project_id TEXT PRIMARY KEY REFERENCES projects(id) ON DELETE CASCADE,
    in_progress_limit INTEGER NOT NULL DEFAULT 4 CHECK (in_progress_limit > 0),
    review_limit INTEGER NOT NULL DEFAULT 3 CHECK (review_limit > 0),
    approved_limit INTEGER NOT NULL DEFAULT 2 CHECK (approved_limit > 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE worker_flow_limits (
    worker_id TEXT PRIMARY KEY,
    max_concurrent_runs INTEGER NOT NULL DEFAULT 4 CHECK (max_concurrent_runs > 0),
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

ALTER TABLE runs ADD COLUMN queued_at TEXT;
UPDATE runs SET queued_at = started_at WHERE queued_at IS NULL;

CREATE UNIQUE INDEX idx_runs_one_active_per_task
    ON runs(task_id) WHERE status IN ('queued', 'running');
CREATE INDEX idx_runs_worker_queue
    ON runs(worker_id, status, queued_at, id);

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
AND NOT (OLD.status = 'ready' AND NEW.status = 'in_progress'
    AND EXISTS (SELECT 1 FROM runs WHERE task_id = OLD.id AND status = 'running'))
BEGIN
    SELECT RAISE(ABORT, 'Cancel or finish the queued run before moving this task.');
END;

CREATE TRIGGER prevent_active_agent_delete
BEFORE DELETE ON agents
WHEN EXISTS (
    SELECT 1 FROM runs WHERE agent_id = OLD.id AND status IN ('queued', 'running')
)
BEGIN
    SELECT RAISE(ABORT, 'Cancel or finish this agent''s queued runs before deleting it.');
END;
