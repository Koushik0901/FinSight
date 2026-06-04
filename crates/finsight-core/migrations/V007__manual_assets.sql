-- V007: manually tracked assets (§4a)
CREATE TABLE manual_assets (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  asset_type  TEXT NOT NULL,
  value_cents INTEGER NOT NULL DEFAULT 0,
  currency    TEXT NOT NULL DEFAULT 'USD',
  notes       TEXT,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
