-- Marks transactions that move money between the user's own accounts (internal
-- transfers, credit-card payments, e-transfers to self) rather than real income
-- or spending. Report income/expense/net-cash-flow sums exclude these so a
-- $3,000 card payment or an internal savings transfer does not inflate both
-- income and spending. The transaction still appears in the account register.
ALTER TABLE transactions ADD COLUMN is_transfer INTEGER NOT NULL DEFAULT 0;
