-- V023: Full SimpleFin connection/institution model and sync metadata extensions.

-- One row per SimpleFin connection (typically one per bridge access URL, but a single
-- bridge can return multiple institution connections). The access URL credentials are
-- stored in the OS keychain; this table only holds a keychain reference.
CREATE TABLE simplefin_connections (
    id              TEXT PRIMARY KEY,
    access_url_ref  TEXT NOT NULL,
    conn_id         TEXT,
    org_id          TEXT,
    org_name        TEXT,
    org_url         TEXT,
    sfin_url        TEXT,
    label           TEXT,
    status          TEXT NOT NULL DEFAULT 'active',
    last_error      TEXT,
    last_synced_at  TEXT,
    created_at      TEXT NOT NULL
);

-- Financial institutions returned by SimpleFin (one per org_id).
CREATE TABLE institutions (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    domain      TEXT,
    sfin_url    TEXT
);

-- Securities referenced by investment account holdings.
CREATE TABLE securities (
    id                   TEXT PRIMARY KEY,
    connection_id        TEXT NOT NULL REFERENCES simplefin_connections(id) ON DELETE CASCADE,
    external_security_id TEXT NOT NULL,
    ticker_symbol        TEXT,
    name                 TEXT,
    currency             TEXT,
    UNIQUE(connection_id, external_security_id)
);

-- Investment holdings per account.
CREATE TABLE holdings (
    id               TEXT PRIMARY KEY,
    account_id       TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    security_id      TEXT NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
    quantity         REAL,
    cost_basis_cents INTEGER,
    market_value_cents INTEGER,
    currency         TEXT,
    UNIQUE(account_id, security_id)
);

-- Link accounts to their SimpleFin connection/institution and capture
-- bridge-specific metadata needed for classification, deduplication, and display.
ALTER TABLE accounts ADD COLUMN connection_id TEXT;
ALTER TABLE accounts ADD COLUMN institution_id TEXT;
ALTER TABLE accounts ADD COLUMN external_account_id TEXT;
ALTER TABLE accounts ADD COLUMN official_name TEXT;
ALTER TABLE accounts ADD COLUMN mask TEXT;
ALTER TABLE accounts ADD COLUMN subtype TEXT;
ALTER TABLE accounts ADD COLUMN account_group TEXT NOT NULL DEFAULT 'other';
ALTER TABLE accounts ADD COLUMN available_balance_cents INTEGER;
ALTER TABLE accounts ADD COLUMN balance_date TEXT;
ALTER TABLE accounts ADD COLUMN extra_json TEXT;
ALTER TABLE accounts ADD COLUMN raw_json TEXT;
ALTER TABLE accounts ADD COLUMN import_pending INTEGER NOT NULL DEFAULT 0;

-- Natural key for SimpleFin account deduplication.
CREATE UNIQUE INDEX idx_accounts_connection_external
    ON accounts(connection_id, external_account_id)
    WHERE archived_at IS NULL;

-- Track where a balance snapshot came from (simplefin, manual, recomputed, etc.).
ALTER TABLE account_balances ADD COLUMN available_balance_cents INTEGER;
ALTER TABLE account_balances ADD COLUMN source TEXT;

-- Raw synced payload for debugging/reconciliation, plus pending flag and external ids.
ALTER TABLE transactions ADD COLUMN raw_synced_data TEXT;
ALTER TABLE transactions ADD COLUMN pending INTEGER NOT NULL DEFAULT 0;
ALTER TABLE transactions ADD COLUMN external_tx_id TEXT;
ALTER TABLE transactions ADD COLUMN external_account_id TEXT;
