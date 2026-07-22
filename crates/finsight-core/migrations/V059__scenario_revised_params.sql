-- Issue #73: revise a saved scenario's assumptions without rebuilding it.
--
-- The original params/result/baseline (V055) stay IMMUTABLE — they are the
-- record of the decision as saved. A revision is a second set of what-if params
-- stored alongside, so the UI can show three distinct things without confusing
-- them: the original result (as saved), the current result (original params vs
-- today's baseline = live-data drift), and the revised result (revised params vs
-- today's baseline = the effect of the assumption edit). NULL = no revision yet.
ALTER TABLE scenarios ADD COLUMN revised_params_json TEXT;
