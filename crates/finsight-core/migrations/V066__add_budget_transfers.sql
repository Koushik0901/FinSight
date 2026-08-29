-- Atomic Cover Ledger (Actual's Cover as auditable row) — FinSight's auditable
-- budget transfer log. Each row moves `amount_cents` from one category to another
-- within a single month. Net effect per category is `+transfers_in - transfers_out`
-- and feeds `available = budgeted + carryover + transfers_in - transfers_out - spent`.
-- See docs/superpowers/plans/2026-08-23-what-to-steal-from-actual.md Task 4.

CREATE TABLE budget_transfers (
  id TEXT PRIMARY KEY,
  month TEXT NOT NULL,
  from_category TEXT,
  to_category TEXT,
  amount_cents INTEGER NOT NULL CHECK(amount_cents > 0),
  note TEXT,
  created_at TEXT NOT NULL,
  CHECK (from_category IS NOT NULL OR to_category IS NOT NULL),
  CHECK (from_category IS NULL OR to_category IS NULL OR from_category != to_category)
);

CREATE INDEX idx_budget_transfers_month ON budget_transfers(month);
CREATE INDEX idx_budget_transfers_from_month ON budget_transfers(from_category, month);
CREATE INDEX idx_budget_transfers_to_month ON budget_transfers(to_category, month);
