-- Drop the `category_id REFERENCES categories(id)` constraint on `rules`.
--
-- V049 gave rules a `treatment` beyond 'categorize' ('transfer' | 'settle_up')
-- so a counterparty verdict persists to future imports. Those two treatments
-- don't categorize anything — `category_id` is only ever read when
-- treatment = 'categorize' (see V049's comment) — so
-- `repos::transactions::apply_verdict_to_matching` upserts transfer/settle_up
-- rules with `category_id = ''`. Under this DB's always-on `foreign_keys`
-- pragma that insert violates `category_id TEXT NOT NULL REFERENCES
-- categories(id)` (no category has id ''). SQLite has no ALTER TABLE to drop
-- a single constraint, so rebuild the table without the REFERENCES clause;
-- NOT NULL and every other column are unchanged.
ALTER TABLE rules RENAME TO rules_v049;

CREATE TABLE rules (
  id          TEXT PRIMARY KEY,
  pattern     TEXT NOT NULL,   -- matched with lower(merchant_raw) LIKE lower(pattern)
  category_id TEXT NOT NULL,   -- read only when treatment = 'categorize'; '' for transfer/settle_up rules
  enabled     INTEGER NOT NULL DEFAULT 1,
  source      TEXT NOT NULL DEFAULT 'user',  -- 'user' | 'agent-proposed'
  created_at  TEXT NOT NULL,
  treatment   TEXT NOT NULL DEFAULT 'categorize'
);

INSERT INTO rules (id, pattern, category_id, enabled, source, created_at, treatment)
SELECT id, pattern, category_id, enabled, source, created_at, treatment FROM rules_v049;

DROP TABLE rules_v049;

CREATE INDEX idx_rules_enabled ON rules(enabled) WHERE enabled = 1;
