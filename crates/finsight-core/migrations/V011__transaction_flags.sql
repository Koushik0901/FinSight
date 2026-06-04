-- V011: per-transaction flags (§5d)
ALTER TABLE transactions ADD COLUMN is_reimbursable INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transactions ADD COLUMN is_split        INTEGER NOT NULL DEFAULT 0;
