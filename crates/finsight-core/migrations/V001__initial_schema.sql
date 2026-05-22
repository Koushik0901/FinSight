-- FinSight initial schema (Phase 1 surface)
-- Money is INTEGER cents. Times are TEXT in ISO-8601 / RFC3339.

CREATE TABLE category_groups (
  id          TEXT PRIMARY KEY,
  label       TEXT NOT NULL,
  hint        TEXT,
  sort_order  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE categories (
  id           TEXT PRIMARY KEY,
  group_id     TEXT NOT NULL REFERENCES category_groups(id),
  label        TEXT NOT NULL,
  color        TEXT NOT NULL,
  icon         TEXT,
  sort_order   INTEGER NOT NULL DEFAULT 0,
  archived_at  TEXT
);
CREATE INDEX idx_categories_group ON categories(group_id) WHERE archived_at IS NULL;

CREATE TABLE merchants (
  id              TEXT PRIMARY KEY,
  canonical_name  TEXT NOT NULL,
  color           TEXT NOT NULL,
  initials        TEXT NOT NULL
);

CREATE TABLE accounts (
  id           TEXT PRIMARY KEY,
  owner        TEXT NOT NULL,
  bank         TEXT NOT NULL,
  type         TEXT NOT NULL,
  name         TEXT NOT NULL,
  last4        TEXT,
  currency     TEXT NOT NULL DEFAULT 'USD',
  color        TEXT NOT NULL,
  archived_at  TEXT,
  created_at   TEXT NOT NULL
);
CREATE INDEX idx_accounts_active ON accounts(owner) WHERE archived_at IS NULL;

CREATE TABLE account_balances (
  account_id    TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  as_of_date    TEXT NOT NULL,
  balance_cents INTEGER NOT NULL,
  PRIMARY KEY (account_id, as_of_date)
);

CREATE TABLE transactions (
  id              TEXT PRIMARY KEY,
  account_id      TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  posted_at       TEXT NOT NULL,
  amount_cents    INTEGER NOT NULL,
  merchant_raw    TEXT NOT NULL,
  merchant_id     TEXT REFERENCES merchants(id),
  category_id    TEXT REFERENCES categories(id),
  status          TEXT NOT NULL DEFAULT 'cleared',
  notes           TEXT,
  ai_confidence   REAL,
  ai_explanation  TEXT,
  is_anomaly      INTEGER NOT NULL DEFAULT 0,
  created_at      TEXT NOT NULL
);
CREATE INDEX idx_txn_timeline ON transactions(posted_at DESC, account_id);
CREATE INDEX idx_txn_category ON transactions(category_id, posted_at);
CREATE INDEX idx_txn_merchant ON transactions(merchant_id);
CREATE INDEX idx_txn_anomaly ON transactions(is_anomaly) WHERE is_anomaly = 1;

CREATE TABLE audit_log (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  at          TEXT NOT NULL,
  actor       TEXT NOT NULL,
  action      TEXT NOT NULL,
  entity      TEXT,
  entity_id   TEXT,
  details     TEXT
);
-- audit_log is created in V001 but the first inserts land in Phase 2
-- (CSV import) and Phase 3 (agent writes).
