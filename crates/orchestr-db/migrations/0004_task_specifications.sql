ALTER TABLE tasks ADD COLUMN acceptance_criteria TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN implementation_notes TEXT;
ALTER TABLE tasks ADD COLUMN relevant_paths TEXT NOT NULL DEFAULT '[]';
ALTER TABLE tasks ADD COLUMN dependency_ids TEXT NOT NULL DEFAULT '[]';
