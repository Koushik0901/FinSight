-- Investment/activity metadata parsed from brokerage CSV exports (e.g.
-- Wealthsimple). Strings are stored provider-verbatim; classification
-- (which activity types imply an internal transfer) lives in Rust
-- (categorize::activity_implies_transfer).
ALTER TABLE transactions ADD COLUMN activity_type TEXT;      -- 'Trade' | 'Dividend' | 'Interest' | 'Tax' | 'MoneyMovement' | ...
ALTER TABLE transactions ADD COLUMN activity_sub_type TEXT;  -- 'BUY' | 'SELL' | 'EFT' | 'E_TRFIN' | 'NRT' | ...
ALTER TABLE transactions ADD COLUMN symbol TEXT;
ALTER TABLE transactions ADD COLUMN security_name TEXT;
ALTER TABLE transactions ADD COLUMN quantity REAL;           -- signed; SELL rows negative
ALTER TABLE transactions ADD COLUMN unit_price REAL;         -- dollars at full export precision

CREATE INDEX idx_transactions_account_activity
    ON transactions(account_id, activity_type)
    WHERE activity_type IS NOT NULL;
