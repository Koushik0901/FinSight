-- Ownership of manually-tracked assets (a house, car, valuables), mirroring
-- account_owners exactly: zero owners = household/unattributable (the honest
-- default for existing assets), 1 = sole, 2+ = jointly owned. share_bps (basis
-- points, NULL = equal split) apportions a jointly-owned asset the same way it
-- apportions a joint account — so a member's net worth folds in THEIR share of a
-- shared house, and two partners' separate apps never double-count it (the
-- unattributed remainder is the residual owned by the other app).
--
-- Additive: no existing asset gets an owner row, so per-member net worth is
-- unchanged until ownership is assigned, and household net worth is untouched.
CREATE TABLE asset_owners (
    asset_id TEXT NOT NULL REFERENCES manual_assets(id) ON DELETE CASCADE,
    member_id TEXT NOT NULL REFERENCES household_members(id) ON DELETE CASCADE,
    share_bps INTEGER,
    PRIMARY KEY (asset_id, member_id)
);
