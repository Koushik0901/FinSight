-- V005: saved what-if scenarios
CREATE TABLE scenarios (
  id          TEXT PRIMARY KEY,
  description TEXT NOT NULL,
  result_json TEXT NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX idx_scenarios_created ON scenarios(created_at);
