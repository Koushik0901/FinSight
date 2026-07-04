-- Household members and account ownership.
-- A household has N members (single person, couple, family, roommates). An
-- account is owned by zero or more members:
--   0 owners  = household / unassigned (the honest default for existing data)
--   1 owner   = sole account
--   2+ owners = JOINT account — jointness is derived from the data, there is
--               no boolean flag to drift out of sync.
-- Ownership shares are equal in v1; a share_pct column can be added later.
-- accounts.owner (TEXT) is kept as a derived display string ("A & B").
CREATE TABLE household_members (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE account_owners (
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    member_id TEXT NOT NULL REFERENCES household_members(id) ON DELETE CASCADE,
    PRIMARY KEY (account_id, member_id)
);
