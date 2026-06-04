-- V008: tracked liabilities (§4b)
CREATE TABLE liabilities (
  id             TEXT PRIMARY KEY,
  name           TEXT NOT NULL,
  liability_type TEXT NOT NULL,
  balance_cents  INTEGER NOT NULL DEFAULT 0,
  limit_cents    INTEGER,
  apr_pct        REAL,
  payoff_date    TEXT,
  currency       TEXT NOT NULL DEFAULT 'USD',
  created_at     TEXT NOT NULL,
  updated_at     TEXT NOT NULL
);
