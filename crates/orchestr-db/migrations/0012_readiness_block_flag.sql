ALTER TABLE tasks ADD COLUMN readiness_blocked INTEGER NOT NULL DEFAULT 0 CHECK (readiness_blocked IN (0, 1));
