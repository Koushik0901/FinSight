-- V021: savings APY and richer loan metadata
ALTER TABLE accounts ADD COLUMN apy_pct REAL;
ALTER TABLE liabilities ADD COLUMN original_balance_cents INTEGER;
ALTER TABLE liabilities ADD COLUMN started_at TEXT;
