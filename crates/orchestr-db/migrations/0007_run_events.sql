CREATE TABLE run_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    run_id TEXT NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    message TEXT NOT NULL,
    command TEXT,
    file_path TEXT,
    exit_code INTEGER,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

INSERT INTO run_events (run_id, kind, message, created_at)
SELECT run_id, 'command.output', text, created_at FROM run_output;

CREATE INDEX idx_run_events_run_id ON run_events(run_id, id ASC);
