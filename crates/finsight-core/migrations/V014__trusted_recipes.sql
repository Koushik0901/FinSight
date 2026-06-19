-- Trusted recipes: recurring AI-assisted planning workflows
CREATE TABLE agent_recipes (
  id              TEXT PRIMARY KEY,
  title           TEXT NOT NULL,
  description     TEXT NOT NULL,
  recipe_kind     TEXT NOT NULL DEFAULT 'custom',
  prompt_template TEXT NOT NULL,
  cadence         TEXT NOT NULL DEFAULT 'monthly',
  day_of_week     INTEGER,
  day_of_month    INTEGER,
  status          TEXT NOT NULL DEFAULT 'active',
  last_run_at     TEXT,
  next_run_at     TEXT,
  run_count       INTEGER NOT NULL DEFAULT 0,
  created_at      TEXT NOT NULL,
  updated_at      TEXT NOT NULL
);
CREATE INDEX idx_recipes_status   ON agent_recipes(status);
CREATE INDEX idx_recipes_next_run ON agent_recipes(next_run_at);

-- Recipe runs: execution history per recipe
CREATE TABLE agent_recipe_runs (
  id           TEXT PRIMARY KEY,
  recipe_id    TEXT NOT NULL REFERENCES agent_recipes(id) ON DELETE CASCADE,
  bundle_id    TEXT REFERENCES agent_action_bundles(id) ON DELETE SET NULL,
  triggered_at TEXT NOT NULL,
  status       TEXT NOT NULL DEFAULT 'running',
  error        TEXT,
  created_at   TEXT NOT NULL
);
CREATE INDEX idx_recipe_runs_recipe ON agent_recipe_runs(recipe_id);
CREATE INDEX idx_recipe_runs_status ON agent_recipe_runs(status);
