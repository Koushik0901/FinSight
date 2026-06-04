-- V010: what the agent has learned from user corrections (§13b)
CREATE TABLE agent_memory (
  id           TEXT PRIMARY KEY,
  kind         TEXT NOT NULL,
  description  TEXT NOT NULL,
  merchant_key TEXT,
  created_at   TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_agent_memory_key ON agent_memory(kind, merchant_key);
