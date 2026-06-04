-- V009: agent-suggested categorization rules awaiting review (§11a)
CREATE TABLE rule_proposals (
  id          TEXT PRIMARY KEY,
  when_label  TEXT NOT NULL,
  description TEXT NOT NULL,
  pattern     TEXT NOT NULL,
  category_id TEXT NOT NULL,
  status      TEXT NOT NULL DEFAULT 'pending',
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_rule_proposals_status ON rule_proposals(status);
