-- Enables scale-up recovery: a single failed slot can be replaced by `count` nodes
-- (default 1 preserves 1:1 replacement behavior for existing rows).
ALTER TABLE recovery_nodes ADD COLUMN count INTEGER NOT NULL DEFAULT 1;
