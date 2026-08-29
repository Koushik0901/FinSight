-- Declarative funding templates (Actual's #template as a table) — ordered by priority.
-- Each row funds one category for a month. See docs/superpowers/plans/2026-08-23-what-to-steal-from-actual.md Task 3.
CREATE TABLE funding_templates (
  id TEXT PRIMARY KEY,
  category_id TEXT NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK(kind IN ('fixed','up_to','by','average','percent','remainder','schedule')),
  params_json TEXT NOT NULL DEFAULT '{}',
  priority INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);
CREATE INDEX idx_funding_templates_priority ON funding_templates(priority ASC, id ASC);
