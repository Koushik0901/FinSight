-- Phase 2 schema additions: import history, mapping cache, accounts.source, settings KV.

CREATE TABLE imports (
  id                       TEXT PRIMARY KEY,
  source                   TEXT NOT NULL,        -- 'csv' | 'manual' | 'sample'
  filename                 TEXT,                 -- NULL for manual/sample
  account_id               TEXT REFERENCES accounts(id),
  started_at               TEXT NOT NULL,
  finished_at              TEXT,                 -- NULL until run completes
  rows_imported            INTEGER NOT NULL DEFAULT 0,
  rows_skipped_duplicates  INTEGER NOT NULL DEFAULT 0,
  error                    TEXT
);
CREATE INDEX idx_imports_unfinished ON imports(finished_at) WHERE finished_at IS NULL;

CREATE TABLE csv_import_mappings (
  account_id    TEXT PRIMARY KEY REFERENCES accounts(id) ON DELETE CASCADE,
  mapping_json  TEXT NOT NULL,
  last_used_at  TEXT NOT NULL
);

ALTER TABLE accounts ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';

CREATE INDEX idx_txn_dedup ON transactions(account_id, posted_at, amount_cents, merchant_raw);

CREATE TABLE settings (
  key    TEXT PRIMARY KEY,
  value  TEXT NOT NULL                            -- JSON-encoded string
);
