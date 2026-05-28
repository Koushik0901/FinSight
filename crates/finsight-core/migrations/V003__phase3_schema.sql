-- V003: categorizations audit trail + rules engine

CREATE TABLE categorizations (
  id          TEXT PRIMARY KEY,
  txn_id      TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id TEXT REFERENCES categories(id),  -- NULL means category cleared by user
  source      TEXT NOT NULL,                   -- 'rule' | 'llm' | 'user'
  confidence  REAL NOT NULL DEFAULT 1.0,
  model       TEXT,                            -- NULL for rule/user assignments
  at          TEXT NOT NULL
);
CREATE INDEX idx_cat_txn ON categorizations(txn_id, at DESC);

CREATE TABLE rules (
  id          TEXT PRIMARY KEY,
  pattern     TEXT NOT NULL,   -- matched with lower(merchant_raw) LIKE lower(pattern)
  category_id TEXT NOT NULL REFERENCES categories(id),
  enabled     INTEGER NOT NULL DEFAULT 1,
  source      TEXT NOT NULL DEFAULT 'user',  -- 'user' | 'agent-proposed'
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_rules_enabled ON rules(enabled) WHERE enabled = 1;
