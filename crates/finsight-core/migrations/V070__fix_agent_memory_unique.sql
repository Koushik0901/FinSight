-- Fix UNIQUE index on (kind, merchant_key) where merchant_key is nullable.
-- In SQLite NULL != NULL, so INSERT ... ON CONFLICT(kind, merchant_key) never conflicts when merchant_key IS NULL,
-- creating duplicate rows for the same preference key stored as NULL. Preferences use merchant_key as key, so duplicates accumulate.
-- Recreate index as COALESCE(merchant_key, '') so NULL and '' are treated as same for uniqueness.
DROP INDEX IF EXISTS idx_agent_memory_key;
CREATE UNIQUE INDEX idx_agent_memory_key ON agent_memory(kind, COALESCE(merchant_key, ''));
