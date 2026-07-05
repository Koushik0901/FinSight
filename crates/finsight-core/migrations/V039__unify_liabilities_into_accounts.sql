-- Unify the standalone `liabilities` table into `accounts`.
--
-- Before this migration, a credit card (or any debt) could be tracked TWO
-- ways at once: as a Credit-type Account (with real transactions, a synced
-- or derived balance) AND as a `liabilities` row (liability_type =
-- 'credit-card', with APR/min-payment/payoff-date). Net worth summed BOTH
-- independently (accounts_sum + assets_sum - liabilities_sum), so a card
-- tracked both ways had its debt subtracted twice. Separately, the debt
-- payoff engine and the Copilot's debt-planning tools only ever read the
-- `liabilities` table, so a real Credit account's balance never
-- participated in debt-snowball/avalanche math unless manually duplicated.
--
-- `accounts` already has Credit and Loan types — it is the natural single
-- home for debt. This migration:
--   1. Adds the debt-specific fields `liabilities` had that `accounts`
--      lacked (apr_pct, min_payment_cents, payoff_date, limit_cents,
--      original_balance_cents, started_at) — the same optional,
--      type-conditional pattern already used for `apy_pct` on Savings.
--   2. Folds every existing `liabilities` row into a new `accounts` row,
--      preserving the row's id so any references (goals) still resolve.
--      Balance sign is flipped: `liabilities.balance_cents` stored the
--      POSITIVE amount owed; `accounts` convention is NEGATIVE for debt.
--   3. Repoints `goals.liability_id` onto the (already-existing)
--      `goals.account_id` column using the preserved ids, then drops the
--      now-empty `liability_id` column and the `liabilities` table.

ALTER TABLE accounts ADD COLUMN apr_pct REAL;
ALTER TABLE accounts ADD COLUMN min_payment_cents INTEGER;
ALTER TABLE accounts ADD COLUMN payoff_date TEXT;
ALTER TABLE accounts ADD COLUMN limit_cents INTEGER;
ALTER TABLE accounts ADD COLUMN original_balance_cents INTEGER;
ALTER TABLE accounts ADD COLUMN started_at TEXT;

INSERT INTO accounts (
    id, owner, bank, type, name, last4, currency, color, source,
    liquidity_type, emergency_fund_eligible, goal_earmark, apy_pct, created_at,
    account_group, apr_pct, min_payment_cents, payoff_date, limit_cents,
    original_balance_cents, started_at
)
SELECT
    id,
    'Household',
    'Manual',
    CASE liability_type
        WHEN 'credit-card' THEN 'Credit'
        WHEN 'mortgage' THEN 'Loan'
        WHEN 'loan' THEN 'Loan'
        ELSE 'Other'
    END,
    name,
    NULL,
    currency,
    CASE liability_type
        WHEN 'credit-card' THEN '#F97316'
        WHEN 'mortgage' THEN '#F87171'
        WHEN 'loan' THEN '#F87171'
        ELSE '#94A3B8'
    END,
    'manual',
    'restricted',
    0,
    NULL,
    NULL,
    created_at,
    'debt',
    apr_pct,
    min_payment_cents,
    payoff_date,
    limit_cents,
    original_balance_cents,
    started_at
FROM liabilities;

INSERT INTO account_balances (account_id, as_of_date, balance_cents, source)
SELECT id, date('now'), -balance_cents, 'manual'
FROM liabilities;

UPDATE goals SET account_id = liability_id
WHERE liability_id IS NOT NULL AND account_id IS NULL;

ALTER TABLE goals DROP COLUMN liability_id;

DROP TABLE liabilities;
