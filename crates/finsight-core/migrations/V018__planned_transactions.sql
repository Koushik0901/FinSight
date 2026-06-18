CREATE TABLE IF NOT EXISTS planned_transactions (
  id           TEXT PRIMARY KEY,
  description  TEXT NOT NULL,
  amount_cents INTEGER NOT NULL,
  account_id   TEXT REFERENCES accounts(id) ON DELETE SET NULL,
  category_id  TEXT REFERENCES categories(id) ON DELETE SET NULL,
  due_date     TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'planned',
  source       TEXT NOT NULL DEFAULT 'agent',
  created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_planned_txn_due ON planned_transactions(due_date);
CREATE INDEX IF NOT EXISTS idx_planned_txn_status ON planned_transactions(status);
