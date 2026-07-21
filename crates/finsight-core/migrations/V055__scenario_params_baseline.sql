-- V055: durable what-if scenarios — store the params and the financial baseline
-- alongside the result so a saved scenario can be recomputed against current
-- data, compared consistently, and checked for staleness. All nullable so the
-- pre-existing result-only rows keep working (they show as "legacy": viewable,
-- but not recomputable/comparable).
ALTER TABLE scenarios ADD COLUMN params_json TEXT;
ALTER TABLE scenarios ADD COLUMN baseline_json TEXT;
ALTER TABLE scenarios ADD COLUMN months INTEGER;
-- Soft-archive: hidden from the active list but preserved for later reference.
ALTER TABLE scenarios ADD COLUMN archived_at TEXT;
