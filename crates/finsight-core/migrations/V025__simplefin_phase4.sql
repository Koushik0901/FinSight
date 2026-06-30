-- Transfer links between synced transactions
CREATE TABLE transaction_transfers (
    id TEXT PRIMARY KEY,
    from_transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    to_transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    confidence TEXT NOT NULL CHECK(confidence IN ('high', 'medium', 'low')),
    detected_at TEXT NOT NULL,
    user_confirmed INTEGER NOT NULL DEFAULT 0,
    UNIQUE(from_transaction_id, to_transaction_id)
);

-- SimpleFin sync alerts (drift, errors, transfer suggestions)
CREATE TABLE simplefin_alerts (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    alert_type TEXT NOT NULL CHECK(alert_type IN ('drift', 'sync_error', 'transfer_suggestion')),
    severity TEXT NOT NULL CHECK(severity IN ('info', 'warning', 'error')),
    message TEXT NOT NULL,
    details_json TEXT,
    acknowledged_at TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX idx_simplefin_alerts_account ON simplefin_alerts(account_id, acknowledged_at, created_at DESC);

-- Investment holdings per account per day.
-- V024 created a holdings table without as_of_date; migrate it to the Phase 4 schema.
ALTER TABLE holdings RENAME TO holdings_v024;

CREATE TABLE holdings (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    security_id TEXT NOT NULL REFERENCES securities(id) ON DELETE CASCADE,
    quantity REAL,
    cost_basis_cents INTEGER,
    market_value_cents INTEGER,
    currency TEXT,
    as_of_date TEXT NOT NULL,
    UNIQUE(account_id, security_id, as_of_date)
);

INSERT INTO holdings (id, account_id, security_id, quantity, cost_basis_cents, market_value_cents, currency, as_of_date)
SELECT id, account_id, security_id, quantity, cost_basis_cents, market_value_cents, currency, date('now')
FROM holdings_v024;

DROP TABLE holdings_v024;
