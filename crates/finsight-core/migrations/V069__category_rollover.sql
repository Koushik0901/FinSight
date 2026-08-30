-- Per-category rollover toggle (B-P1-1).
-- Whether unspent budget should roll forward as carryover into next month.
-- Default 1 so every existing category keeps its current behaviour — rollover is
-- automatic unless the user explicitly disables it for a category (e.g.
-- reimbursables or pay-yourself-first envelopes that should reset monthly).
ALTER TABLE categories ADD COLUMN rollover_enabled INTEGER NOT NULL DEFAULT 1;
-- 0 = reset each month (carryover = 0), 1 = carry budgeted - spent forward
