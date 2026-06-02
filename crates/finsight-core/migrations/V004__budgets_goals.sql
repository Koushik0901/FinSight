-- V004: envelope budgets + savings goals

-- Monthly budget amounts per category.
-- month is stored as 'YYYY-MM'.
CREATE TABLE budgets (
  id          TEXT PRIMARY KEY,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  month       TEXT NOT NULL,          -- 'YYYY-MM'
  amount_cents INTEGER NOT NULL DEFAULT 0,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL,
  UNIQUE (category_id, month)
);
CREATE INDEX idx_budgets_month ON budgets(month);

-- Savings goals / sinking funds / debt payoff targets.
CREATE TABLE goals (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  type          TEXT NOT NULL DEFAULT 'save-by-date',
  -- 'save-by-date' | 'build-balance' | 'debt-payoff' | 'spending-cap'
  target_cents  INTEGER NOT NULL DEFAULT 0,
  current_cents INTEGER NOT NULL DEFAULT 0,
  monthly_cents INTEGER NOT NULL DEFAULT 0,  -- planned monthly contribution
  target_date   TEXT,                         -- ISO date or NULL
  color         TEXT NOT NULL DEFAULT '#C9F950',
  notes         TEXT,
  sort_order    INTEGER NOT NULL DEFAULT 0,
  archived_at   TEXT,
  created_at    TEXT NOT NULL
);
