-- Issue #75: subscription lifecycle — trials, cancellation, duplicates.
--
-- Follow-up to #58's subscription_overrides (V057), which held only a
-- confirm/dismiss verdict. #75 adds three lifecycle facts a user records against
-- the same canonical merchant key:
--   * a TRIAL that will convert on trial_ends_at (heads-up before it charges),
--   * a CANCELLATION on cancelled_at (so a later charge reads as "you thought
--     this was cancelled"), which needs a `cancelled` verdict distinct from
--     `dismissed` (dismissed = "not a subscription / don't alert"; cancelled =
--     "a real subscription I ended"),
--   * a `label` captured at mark time, so a trial reminder still names the
--     service even if the series has too few charges to be re-detected.
--
-- SQLite can't widen a CHECK in place, so recreate the table and copy the
-- existing verdicts forward. `label`, `trial_ends_at`, `cancelled_at` are
-- nullable — a row may carry any subset (e.g. a confirmed sub that is also a
-- trial). verdict stays NOT NULL; marking a trial with no prior verdict implies
-- 'confirmed' (marking a trial affirms it is a subscription).
CREATE TABLE subscription_overrides_new (
    merchant_key TEXT PRIMARY KEY,
    verdict TEXT NOT NULL CHECK (verdict IN ('confirmed', 'dismissed', 'cancelled')),
    label TEXT,
    trial_ends_at TEXT,
    cancelled_at TEXT,
    created_at TEXT NOT NULL
);

INSERT INTO subscription_overrides_new (merchant_key, verdict, created_at)
    SELECT merchant_key, verdict, created_at FROM subscription_overrides;

DROP TABLE subscription_overrides;
ALTER TABLE subscription_overrides_new RENAME TO subscription_overrides;
