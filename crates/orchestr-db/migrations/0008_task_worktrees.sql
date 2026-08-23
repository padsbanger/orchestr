ALTER TABLE tasks ADD COLUMN branch TEXT;
ALTER TABLE tasks ADD COLUMN worktree_path TEXT;

CREATE UNIQUE INDEX idx_tasks_branch ON tasks(branch) WHERE branch IS NOT NULL;
CREATE UNIQUE INDEX idx_tasks_worktree_path ON tasks(worktree_path) WHERE worktree_path IS NOT NULL;
