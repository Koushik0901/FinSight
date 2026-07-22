-- Issue #58: durable user verdicts on detected subscriptions.
--
-- Subscription detection is DERIVED at read time (recurring.rs), so a user's
-- decision to confirm a detection or dismiss a false positive has nowhere to
-- live in the transaction rows. This table holds that verdict, keyed by the
-- same canonical merchant key the detector groups on, so a dismissed series
-- stops producing price-change / renewal notifications and reads as dismissed
-- in the UI. Precedent: anomaly_dismissed (V041), transfer_override (V046).
CREATE TABLE subscription_overrides (
    merchant_key TEXT PRIMARY KEY,
    -- 'confirmed' = the user affirms this is a real subscription;
    -- 'dismissed' = ignore it (not a subscription, or don't alert me).
    verdict TEXT NOT NULL CHECK (verdict IN ('confirmed', 'dismissed')),
    created_at TEXT NOT NULL
);
