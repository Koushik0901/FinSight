-- V006: daily net-worth snapshots for the Today net-worth chart (§3a)
CREATE TABLE net_worth_snapshots (
  id          TEXT PRIMARY KEY,
  date        TEXT NOT NULL UNIQUE,   -- ISO date 'YYYY-MM-DD'
  total_cents INTEGER NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_nws_date ON net_worth_snapshots(date);
