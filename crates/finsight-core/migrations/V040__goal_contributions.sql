-- Append-only ledger of contributions to (and withdrawals from) a goal. A
-- manual goal's `current_cents` becomes a derived total — the sum of its
-- contributions — instead of a mutable number that could drift or be silently
-- overwritten by the account-balance sync. Account-linked goals keep deriving
-- their balance from the linked account and do not use this ledger.
CREATE TABLE goal_contributions (
  id           TEXT PRIMARY KEY,
  goal_id      TEXT NOT NULL REFERENCES goals(id) ON DELETE CASCADE,
  -- Positive = money added, negative = withdrawn. Summed to get current_cents.
  amount_cents INTEGER NOT NULL,
  note         TEXT,
  -- How the contribution originated: 'manual', 'opening' (backfill), 'sweep', etc.
  source       TEXT NOT NULL DEFAULT 'manual',
  created_at   TEXT NOT NULL
);

CREATE INDEX idx_goal_contributions_goal ON goal_contributions(goal_id, created_at);

-- Seed the ledger for existing MANUAL goals so the sum matches the balance they
-- already show. Account-linked goals are intentionally excluded — their balance
-- comes from the account, not the ledger.
INSERT INTO goal_contributions(id, goal_id, amount_cents, note, source, created_at)
SELECT lower(hex(randomblob(16))), id, current_cents, 'Opening balance', 'opening', created_at
FROM goals
WHERE account_id IS NULL AND current_cents <> 0;
