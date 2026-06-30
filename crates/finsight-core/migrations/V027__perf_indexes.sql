-- Performance indexes for filtered transaction queries
CREATE INDEX IF NOT EXISTS idx_transactions_posted_at ON transactions(posted_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_account_posted ON transactions(account_id, posted_at DESC);
