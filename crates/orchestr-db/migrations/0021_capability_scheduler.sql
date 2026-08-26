ALTER TABLE tasks ADD COLUMN required_capabilities TEXT NOT NULL DEFAULT '[]';

DROP TRIGGER prevent_active_task_edit;
CREATE TRIGGER prevent_active_task_edit
BEFORE UPDATE OF title, description, acceptance_criteria, implementation_notes,
    relevant_paths, dependency_ids, assigned_agent_id, priority, milestone_id, epic_id,
    required_capabilities ON tasks
WHEN EXISTS (
    SELECT 1 FROM runs WHERE task_id = OLD.id AND status IN ('queued', 'running')
)
BEGIN
    SELECT RAISE(ABORT, 'Cancel or finish the queued run before editing this task.');
END;

CREATE TABLE scheduler_decisions (
    id TEXT PRIMARY KEY NOT NULL,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    task_id TEXT REFERENCES tasks(id) ON DELETE SET NULL,
    worker_id TEXT,
    run_id TEXT REFERENCES runs(id) ON DELETE SET NULL,
    outcome TEXT NOT NULL CHECK (outcome IN ('scheduled', 'skipped', 'blocked')),
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_scheduler_decisions_project_created
    ON scheduler_decisions(project_id, created_at DESC, id DESC);
