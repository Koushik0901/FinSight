-- Restoration envelopes: money that left a pot with the intent to put it back.
--
-- A notional running tab against an intention, NOT a claim about where physical
-- dollars are sitting. Once $10,000 lands in checking it mixes with everything
-- already there, so "which dollars left next" is not a fact in the data — it is
-- a convention you would have to invent. This models the only legs that are
-- actually knowable: what left, and what has gone back.
--
-- Deliberately narrow. Features that need ongoing manual attribution have a poor
-- survival rate past the first month, so an envelope is designed to be opened,
-- reconciled and CLOSED over a few weeks rather than kept forever.

CREATE TABLE restoration_envelopes (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    -- The pot the money came out of, and where it landed. Both optional: a
    -- cross-bank move may not present as a linkable pair, and the envelope is
    -- still worth keeping without them.
    source_account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
    destination_account_id TEXT REFERENCES accounts(id) ON DELETE SET NULL,
    original_cents INTEGER NOT NULL,
    -- ISO date the money left. Anchors the low-point bound: the destination
    -- account's trough SINCE this date is the ceiling on what can still be held.
    opened_on TEXT NOT NULL,
    -- The `%name%` counterparty pattern this envelope expects to collect from,
    -- if any. One person, deliberately: the worked case is a single friend, and
    -- guessing across several would produce a confident wrong "collectable".
    counterparty_pattern TEXT,
    -- Set when the user has reconciled and finished with it. A closed envelope
    -- stops being nagged about.
    closed_at TEXT,
    note TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_restoration_envelopes_open
    ON restoration_envelopes(closed_at, opened_on);

-- Money that has gone BACK into the pot, attributed to an envelope.
--
-- `transaction_id` is optional and nullable on purpose: a clean same-bank
-- transfer is recognisable, but a cross-bank move, one carrying a fee, or a
-- restoration made in several chunks may never present as a single matchable
-- event. A leg the user asserts is still worth recording.
CREATE TABLE restoration_legs (
    id TEXT PRIMARY KEY,
    envelope_id TEXT NOT NULL
        REFERENCES restoration_envelopes(id) ON DELETE CASCADE,
    transaction_id TEXT REFERENCES transactions(id) ON DELETE SET NULL,
    amount_cents INTEGER NOT NULL,
    noted_on TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_restoration_legs_envelope ON restoration_legs(envelope_id);
