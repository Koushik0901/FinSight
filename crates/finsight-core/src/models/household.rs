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
}

/// One (account, member) ownership pair. The full list lets the UI derive
/// sole/joint badges and per-member net-worth attribution in one query.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountOwner {
    pub account_id: String,
    pub member_id: String,
}
