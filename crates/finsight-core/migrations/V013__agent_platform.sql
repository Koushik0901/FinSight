-- Agent sessions: top-level planning conversations
CREATE TABLE agent_sessions (
  id          TEXT PRIMARY KEY,
  title       TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'active',
  task_type   TEXT NOT NULL DEFAULT 'general',
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);
CREATE INDEX idx_agent_sessions_status ON agent_sessions(status);

-- Context snapshots: compressed financial state for a session turn
CREATE TABLE agent_context_snapshots (
  id           TEXT PRIMARY KEY,
  session_id   TEXT NOT NULL REFERENCES agent_sessions(id) ON DELETE CASCADE,
  context_json TEXT NOT NULL,
  created_at   TEXT NOT NULL
);
CREATE INDEX idx_ctx_snapshot_session ON agent_context_snapshots(session_id);

-- Action bundles: proposed changes grouped for user review
CREATE TABLE agent_action_bundles (
  id           TEXT PRIMARY KEY,
  session_id   TEXT REFERENCES agent_sessions(id) ON DELETE SET NULL,
  title        TEXT NOT NULL,
  summary      TEXT NOT NULL,
  rationale    TEXT NOT NULL,
  confidence   REAL NOT NULL DEFAULT 0.0,
  status       TEXT NOT NULL DEFAULT 'pending',
  provider_id  TEXT,
  model_id     TEXT,
  created_at   TEXT NOT NULL,
  updated_at   TEXT NOT NULL
);
CREATE INDEX idx_bundle_session ON agent_action_bundles(session_id);
CREATE INDEX idx_bundle_status  ON agent_action_bundles(status);

-- Action items: individual typed executable changes within a bundle
CREATE TABLE agent_action_items (
  id                TEXT PRIMARY KEY,
  bundle_id         TEXT NOT NULL REFERENCES agent_action_bundles(id) ON DELETE CASCADE,
  action_kind       TEXT NOT NULL,
  payload_json      TEXT NOT NULL,
  preview_json      TEXT,
  rationale         TEXT NOT NULL,
  confidence        REAL NOT NULL DEFAULT 0.0,
  status            TEXT NOT NULL DEFAULT 'pending',
  validation_errors TEXT,
  sort_order        INTEGER NOT NULL DEFAULT 0,
  created_at        TEXT NOT NULL,
  updated_at        TEXT NOT NULL
);
CREATE INDEX idx_item_bundle ON agent_action_items(bundle_id);
CREATE INDEX idx_item_status  ON agent_action_items(status);

-- Execution log: immutable audit trail
CREATE TABLE agent_execution_log (
  id           TEXT PRIMARY KEY,
  item_id      TEXT NOT NULL REFERENCES agent_action_items(id),
  bundle_id    TEXT NOT NULL REFERENCES agent_action_bundles(id),
  action_kind  TEXT NOT NULL,
  status       TEXT NOT NULL,
  result_json  TEXT,
  error        TEXT,
  executed_at  TEXT NOT NULL
);
CREATE INDEX idx_exec_bundle ON agent_execution_log(bundle_id);
CREATE INDEX idx_exec_item   ON agent_execution_log(item_id);
