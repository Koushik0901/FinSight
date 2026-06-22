-- V019: minimum payment tracking for debt planning
ALTER TABLE liabilities ADD COLUMN min_payment_cents INTEGER;
