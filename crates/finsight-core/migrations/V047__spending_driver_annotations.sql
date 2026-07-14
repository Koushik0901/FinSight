-- Sticky user verdicts on a spending "driver" (a normalized merchant). Lets the
-- engine LEARN the user's life: a flagged driver marked one_off / expected /
-- investment stops being treated as a recurring lever, across chat + screen +
-- future recomputes. Keyed by canonical_merchant_key (the same clustering key
-- the baseline/decompose use). Mirrors the transfer_override sticky-verdict idea.
CREATE TABLE spending_driver_annotations (
    merchant_key TEXT PRIMARY KEY,
    verdict      TEXT NOT NULL,          -- 'one_off' | 'expected' | 'investment'
    note         TEXT,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
