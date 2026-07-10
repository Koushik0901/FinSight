-- P2-6: let the user mark a flagged anomaly as reviewed-and-fine. The anomaly
-- detector clears and re-flags every recompute, so without a persistent marker a
-- dismissed anomaly would reappear on the next import. `anomaly_dismissed` is
-- respected by the detector (a dismissed row still counts toward its merchant's
-- baseline, but is never re-flagged), keeping the Insights anomaly feed trustworthy.
ALTER TABLE transactions ADD COLUMN anomaly_dismissed INTEGER NOT NULL DEFAULT 0;

-- Partial index: the feed and detector only ever care about the handful of
-- dismissed rows, so index just those.
CREATE INDEX idx_txn_anomaly_dismissed ON transactions(anomaly_dismissed) WHERE anomaly_dismissed = 1;
