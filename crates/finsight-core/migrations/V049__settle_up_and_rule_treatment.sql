-- Settle-up treatment: a person-to-person flow whose inflows NET against
-- expense (never income). Reimbursement/shared-cost model. Metrics interpret it.
ALTER TABLE transactions ADD COLUMN settle_up INTEGER NOT NULL DEFAULT 0;

-- A rule can now carry a treatment beyond categorization, so a per-counterparty
-- verdict persists to future imports: 'categorize' (default, existing) |
-- 'transfer' | 'settle_up'. category_id is only read when treatment='categorize'.
ALTER TABLE rules ADD COLUMN treatment TEXT NOT NULL DEFAULT 'categorize';
