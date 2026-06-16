CREATE TABLE transaction_splits (
  id           TEXT    PRIMARY KEY,
  txn_id       TEXT    NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
  category_id  TEXT    REFERENCES categories(id),
  amount_cents INTEGER NOT NULL
);
CREATE INDEX idx_splits_txn ON transaction_splits(txn_id);
