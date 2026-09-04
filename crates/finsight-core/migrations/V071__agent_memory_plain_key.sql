-- V071: make the agent_memory uniqueness index directly targetable by upserts.
-- V070 used an expression index (kind, COALESCE(merchant_key, '')) so NULL and ''
-- keys dedupe alike, but SQLite cannot match an expression index from an
-- INSERT ... ON CONFLICT(kind, merchant_key) clause in the bundled SQLite
-- version — every write failed with "ON CONFLICT clause does not match any
-- PRIMARY KEY or UNIQUE constraint". Backfill legacy NULL keys to '' so a
-- plain unique index keeps the same dedupe semantics and stays targetable.
UPDATE agent_memory SET merchant_key = '' WHERE merchant_key IS NULL;
DROP INDEX IF EXISTS idx_agent_memory_key;
CREATE UNIQUE INDEX idx_agent_memory_key ON agent_memory(kind, merchant_key);
