-- Issue #59: evolve the one-shot monthly_reviews snapshot into a durable
-- month-end CLOSE with a lifecycle.
--
-- The original table (V028) recomputed and OVERWROTE its snapshot on every save
-- (INSERT OR REPLACE + UNIQUE(year,month)), which is the opposite of what a
-- trustworthy close needs. We keep the same table (one authoritative notion of
-- "month review") and add the durability the scenario snapshots (V055) model:
--
--   status       — 'in_progress' | 'completed' | 'skipped'. The persisted
--                  in_progress row IS the pause; resume just reopens the screen.
--   completed_at — when the snapshot was frozen ("recorded at"); distinct from
--                  created_at (when the close was started).
--   baseline_json — the metrics baseline the frozen snapshot was computed
--                   against, so later drift ("recorded then vs recomputed now")
--                   can be shown without ever mutating the recorded values.
--   close_json    — the data-quality warnings captured at close plus the user's
--                   acknowledgements and decisions.
--
-- Existing rows were finished snapshots, so they backfill to 'completed' with
-- completed_at = created_at; their baseline/close JSON stays NULL (viewable,
-- but not drift-comparable — the same "legacy" degradation V055 used).
ALTER TABLE monthly_reviews ADD COLUMN status TEXT NOT NULL DEFAULT 'completed';
ALTER TABLE monthly_reviews ADD COLUMN completed_at TEXT;
ALTER TABLE monthly_reviews ADD COLUMN baseline_json TEXT;
ALTER TABLE monthly_reviews ADD COLUMN close_json TEXT;

UPDATE monthly_reviews SET completed_at = created_at WHERE completed_at IS NULL;
