-- Goal contribution cadence: monthly | weekly | biweekly.
--
-- A goal is a stream of contributions, not just a target amount.
-- Before this, every goal assumed a monthly cadence — `monthly_cents` meant
-- "this much per month" and every projection (ETA, required monthly,
-- allocation) divided by that. A user paid weekly or biweekly who entered
-- "$100" was modeled as $100/month instead of ~$433/$217 — their ETA was ~4×
-- too long and their horizon was wrong.
--
-- `period` records the cadence the entered amount corresponds to; the derived
-- monthly equivalent (amount * periods_per_year / 12, rounded) is what
-- planning uses. Monthly stays the default so every existing goal keeps its
-- exact prior behaviour without rewriting.

ALTER TABLE goals ADD COLUMN period TEXT NOT NULL DEFAULT 'monthly';
-- 'monthly' | 'weekly' | 'biweekly'
