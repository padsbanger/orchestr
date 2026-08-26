ALTER TABLE remote_workers ADD COLUMN providers TEXT NOT NULL DEFAULT '[]';

CREATE TABLE worker_management (
    worker_id TEXT PRIMARY KEY NOT NULL,
    display_name TEXT NOT NULL,
    labels TEXT NOT NULL DEFAULT '[]',
    maintenance INTEGER NOT NULL DEFAULT 0 CHECK (maintenance IN (0, 1)),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO worker_management (worker_id, display_name)
SELECT id, name FROM remote_workers;
