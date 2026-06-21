-- SimpleFin bank sync support

ALTER TABLE accounts ADD COLUMN simplefin_account_id TEXT;
ALTER TABLE accounts ADD COLUMN last_synced_at TEXT;
ALTER TABLE accounts ADD COLUMN nickname TEXT;

ALTER TABLE transactions ADD COLUMN imported_id TEXT;
ALTER TABLE transactions ADD COLUMN source TEXT;

CREATE INDEX idx_txn_imported_id ON transactions(account_id, imported_id);
