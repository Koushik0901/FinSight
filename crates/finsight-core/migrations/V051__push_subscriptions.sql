-- Web Push subscriptions for the installed PWA. One row per device/browser that
-- opted in, living in the user's own encrypted DB — a subscription endpoint is
-- a capability to reach that person's phone, so it belongs with their data and
-- not in the shared registry.
--
-- `endpoint` is the push-service URL the browser issued and is globally unique
-- per subscription, so it is the natural identity. Browsers silently rotate
-- subscriptions, and UNIQUE + upsert-on-endpoint means a re-subscribe replaces
-- the row instead of accumulating dead duplicates that every send would retry.
CREATE TABLE push_subscriptions (
  id           TEXT PRIMARY KEY,
  endpoint     TEXT NOT NULL UNIQUE,
  -- Base64url public key + auth secret from PushSubscription.getKey(); the
  -- payload is encrypted to these, so they are useless without the endpoint.
  p256dh       TEXT NOT NULL,
  auth         TEXT NOT NULL,
  -- Purely so a user can tell their devices apart when revoking one.
  label        TEXT,
  created_at   TEXT NOT NULL,
  last_used_at TEXT
);
