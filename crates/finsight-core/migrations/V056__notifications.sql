-- V056: unified notification policy.
-- One table every notification-producing feature routes through, so priority,
-- privacy, dedup, and resolution are applied in one place instead of each
-- feature inventing its own. `dedup_key` suppresses repeats of the same
-- condition/event; `resolved_at` clears a standing condition when it no longer
-- holds; `delivered_at` NULL means held (quiet hours) or suppressed.
CREATE TABLE notifications (
    id           TEXT PRIMARY KEY,
    category     TEXT NOT NULL,          -- cashflow_risk | stale_data | subscription_change | …
    urgency      TEXT NOT NULL,          -- critical | normal | low
    dedup_key    TEXT NOT NULL,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,          -- safe framing, no sensitive figures
    sensitive    TEXT,                   -- optional amount/merchant, redacted per privacy level
    route        TEXT,                   -- in-app route to open, if any
    created_at   TEXT NOT NULL,
    delivered_at TEXT,                   -- when actually surfaced/pushed (NULL = held/suppressed)
    read_at      TEXT,                   -- when the user saw it in the center
    resolved_at  TEXT,                   -- when the underlying condition cleared/expired
    expires_at   TEXT                    -- optional self-expiry for discrete events
);

-- Dedup lookup is only ever over UNRESOLVED rows for a key.
CREATE INDEX idx_notifications_dedup ON notifications(dedup_key, resolved_at);
-- History + unread queries are ordered by recency.
CREATE INDEX idx_notifications_created ON notifications(created_at);
