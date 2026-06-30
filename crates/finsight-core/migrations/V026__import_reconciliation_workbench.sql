-- Durable import reconciliation workbench, sync run audit, and balance-source separation.

ALTER TABLE account_balances RENAME TO account_balances_v026_old;

CREATE TABLE account_balances (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    as_of_date TEXT NOT NULL,
    balance_cents INTEGER NOT NULL,
    available_balance_cents INTEGER,
    source TEXT NOT NULL DEFAULT 'manual',
    PRIMARY KEY (account_id, as_of_date, source)
);

INSERT INTO account_balances (
    account_id,
    as_of_date,
    balance_cents,
    available_balance_cents,
    source
)
SELECT
    account_id,
    as_of_date,
    balance_cents,
    available_balance_cents,
    COALESCE(source, 'manual')
FROM account_balances_v026_old;

DROP TABLE account_balances_v026_old;

CREATE TABLE import_candidates (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL CHECK(source IN ('csv', 'simplefin')),
    import_id TEXT,
    sync_run_id TEXT,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    candidate_json TEXT NOT NULL,
    raw_payload_json TEXT,
    imported_id TEXT,
    external_tx_id TEXT,
    external_account_id TEXT,
    posted_at TEXT NOT NULL,
    amount_cents INTEGER NOT NULL,
    merchant_raw TEXT NOT NULL,
    confidence INTEGER NOT NULL DEFAULT 0,
    reason TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK(status IN ('pending', 'resolved', 'dismissed')),
    resolution TEXT,
    resolved_transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX idx_import_candidates_status_created
    ON import_candidates(status, created_at DESC);
CREATE INDEX idx_import_candidates_account_status
    ON import_candidates(account_id, status);
CREATE INDEX idx_import_candidates_source_status
    ON import_candidates(source, status);
CREATE INDEX idx_import_candidates_sync_run
    ON import_candidates(sync_run_id);

CREATE TABLE import_candidate_matches (
    id TEXT PRIMARY KEY,
    candidate_id TEXT NOT NULL REFERENCES import_candidates(id) ON DELETE CASCADE,
    transaction_id TEXT NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    match_kind TEXT NOT NULL,
    score INTEGER NOT NULL,
    is_recommended INTEGER NOT NULL DEFAULT 0,
    explanation_json TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_import_candidate_matches_candidate_score
    ON import_candidate_matches(candidate_id, score DESC);
CREATE INDEX idx_import_candidate_matches_transaction
    ON import_candidate_matches(transaction_id);

CREATE TABLE sync_runs (
    id TEXT PRIMARY KEY,
    trigger TEXT NOT NULL CHECK(trigger IN ('manual', 'background', 'initial')),
    status TEXT NOT NULL CHECK(status IN ('running', 'success', 'partial', 'failed')),
    started_at TEXT NOT NULL,
    finished_at TEXT,
    accounts_total INTEGER NOT NULL DEFAULT 0,
    accounts_succeeded INTEGER NOT NULL DEFAULT 0,
    accounts_failed INTEGER NOT NULL DEFAULT 0,
    added INTEGER NOT NULL DEFAULT 0,
    updated INTEGER NOT NULL DEFAULT 0,
    skipped INTEGER NOT NULL DEFAULT 0,
    queued_for_review INTEGER NOT NULL DEFAULT 0,
    error_summary TEXT
);

CREATE INDEX idx_sync_runs_started
    ON sync_runs(started_at DESC);

CREATE INDEX idx_transactions_reconciliation_lookup
    ON transactions(account_id, amount_cents, posted_at);

CREATE INDEX idx_transactions_imported_id_lookup
    ON transactions(account_id, imported_id)
    WHERE imported_id IS NOT NULL;
