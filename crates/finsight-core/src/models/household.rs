use serde::{Deserialize, Serialize};
use specta::Type;

/// A person in the household (single user, partner, family member, roommate).
/// Accounts are owned by zero or more members; 2+ owners = a joint account.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HouseholdMember {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    /// True for the one member who operates this install (the "self"). At most
    /// one member is self; drives "my finances" views and self-transfer
    /// recognition. Zero members ⇒ no self, behaves as a solo household.
    pub is_self: bool,
}

/// One (account, member) ownership pair with the member's explicit share, if
/// any. The full list lets the UI derive sole/joint badges and per-member
/// net-worth attribution in one query. `share_bps` None = equal split.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountOwner {
    pub account_id: String,
    pub member_id: String,
    pub share_bps: Option<i64>,
}

/// One (asset, member) ownership pair — the manual-asset analogue of
/// [`AccountOwner`].
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AssetOwner {
    pub asset_id: String,
    pub member_id: String,
    pub share_bps: Option<i64>,
}

/// An owner and their explicit ownership share (basis points, 10000 = 100%) for
/// an account or asset. `share_bps` None ⇒ equal split with the other owners.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OwnerShare {
    pub member_id: String,
    pub share_bps: Option<i64>,
}
