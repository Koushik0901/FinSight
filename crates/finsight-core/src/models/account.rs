use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum AccountType {
    Checking,
    Savings,
    Credit,
    Investment,
    Cash,
    Loan,
    Other,
}

impl AccountType {
    pub fn as_db(&self) -> &'static str {
        match self {
            Self::Checking => "Checking",
            Self::Savings => "Savings",
            Self::Credit => "Credit",
            Self::Investment => "Investment",
            Self::Cash => "Cash",
            Self::Loan => "Loan",
            Self::Other => "Other",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "Checking" => Self::Checking,
            "Savings" => Self::Savings,
            "Credit" => Self::Credit,
            "Investment" => Self::Investment,
            "Cash" => Self::Cash,
            "Loan" => Self::Loan,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Account {
    pub id: String,
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub last4: Option<String>,
    pub currency: String,
    pub color: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub liquidity_type: String,
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
    pub created_at: DateTime<Utc>,
    pub simplefin_account_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub nickname: Option<String>,
    pub connection_id: Option<String>,
    pub institution_id: Option<String>,
    pub external_account_id: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub subtype: Option<String>,
    pub account_group: String,
    pub available_balance_cents: Option<i64>,
    pub balance_date: Option<DateTime<Utc>>,
    pub extra_json: Option<String>,
    pub raw_json: Option<String>,
    pub import_pending: bool,
    /// Debt fields, meaningful only for Credit/Loan-type accounts (shown
    /// conditionally in the UI, mirroring `apy_pct` for Savings). Together
    /// these let a Credit/Loan account fully replace the standalone
    /// liability-tracking model: a debt is just an account with a negative
    /// balance and these optional details, not a separate entity.
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub payoff_date: Option<String>,
    pub limit_cents: Option<i64>,
    pub original_balance_cents: Option<i64>,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct AccountSummary {
    pub id: String,
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub balance_cents: i64,
    /// False when `balance_cents` is just the untouched account-creation seed
    /// value and the account has transaction activity that could have moved
    /// the real balance since then. The UI must not present `balance_cents`
    /// as a trustworthy current balance when this is false.
    #[serde(default = "default_balance_known")]
    pub balance_known: bool,
    /// Source of the snapshot `balance_cents` came from: `simplefin` (bank-
    /// reported), `manual` (legacy user-stamped), `derived` (computed from
    /// opening + activity), or `seed` (untouched opening). Lets the UI show the
    /// balance's basis — "synced", "estimated from your transactions", etc.
    #[serde(default)]
    pub balance_source: Option<String>,
    pub currency: String,
    pub color: String,
    pub source: String,
    #[serde(default = "default_liquidity_type")]
    pub liquidity_type: String,
    #[serde(default = "default_emergency_fund_eligible")]
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
    pub simplefin_account_id: Option<String>,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub nickname: Option<String>,
    pub connection_id: Option<String>,
    pub institution_id: Option<String>,
    pub external_account_id: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub subtype: Option<String>,
    pub account_group: String,
    pub available_balance_cents: Option<i64>,
    pub balance_date: Option<DateTime<Utc>>,
    pub extra_json: Option<String>,
    pub raw_json: Option<String>,
    pub import_pending: bool,
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub payoff_date: Option<String>,
    pub limit_cents: Option<i64>,
    pub original_balance_cents: Option<i64>,
    pub started_at: Option<String>,
}

fn default_source() -> String {
    "manual".to_string()
}

fn default_liquidity_type() -> String {
    "liquid".to_string()
}

fn default_emergency_fund_eligible() -> bool {
    true
}

fn default_balance_known() -> bool {
    true
}

fn default_account_group() -> String {
    "other".to_string()
}

fn default_import_pending() -> bool {
    false
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalancePoint {
    pub date: String,
    pub balance_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountSparkline {
    pub account_id: String,
    pub points: Vec<AccountBalancePoint>,
}

/// How trustworthy the *level* of a reconstructed balance curve is.
///
/// A reconstruction is `opening + Σ(cleared activity)`, so its SHAPE (when the
/// balance rose and fell, and therefore when it peaked) is correct regardless of
/// the anchor. Its LEVEL is only as good as that opening figure.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BalanceAnchorQuality {
    /// A real balance was confirmed for this account — bank-reported, or pinned
    /// by the user — so the curve is calibrated to a known value.
    Calibrated,
    /// The opening anchor is meaningful: either a non-zero opening balance, or a
    /// zero opening on an account with no history predating it (it really did
    /// start empty).
    AnchoredOpening,
    /// The opening is zero AND transactions predate it, so the account's history
    /// was imported behind an anchor that never accounted for it. Movement and
    /// timing are right; every absolute figure is off by an unknown constant.
    AssumedZero,
}

/// A reconstructed balance curve for one account, derived from its opening
/// anchor plus cleared transaction activity.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountBalanceTimeline {
    pub account_id: String,
    pub account_name: String,
    /// End-of-day balance for each day the account saw activity, ascending.
    /// Empty when the account is not reconstructable.
    pub points: Vec<AccountBalancePoint>,
    /// Highest and lowest end-of-day balance within the requested window.
    /// Ties resolve to the earliest date. `None` when there are no points.
    pub peak: Option<AccountBalancePoint>,
    pub trough: Option<AccountBalancePoint>,
    /// Balance at the end of the series — the reconstruction's answer for today.
    pub current_cents: i64,
    pub anchor: BalanceAnchorQuality,
    /// Earliest cleared transaction *included in this curve*. History cannot
    /// reach back further, so a peak before it would be invisible.
    pub earliest_txn_date: Option<String>,
    /// False when the balance cannot be honestly derived from the ledger. See
    /// [`AccountBalanceTimeline::skip_reason`] for which case applies.
    pub reconstructable: bool,
    /// Why reconstruction was refused, phrased for a human (or a model) to
    /// relay. `None` when `reconstructable` is true.
    pub skip_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Type)]
pub struct AccountPatch {
    pub name: Option<String>,
    pub bank: Option<String>,
    pub account_type: Option<AccountType>,
    pub color: Option<String>,
    pub last4: Option<Option<String>>,
    pub currency: Option<String>,
    pub liquidity_type: Option<String>,
    pub emergency_fund_eligible: Option<bool>,
    pub goal_earmark: Option<Option<String>>,
    pub apy_pct: Option<Option<f64>>,
    pub nickname: Option<Option<String>>,
    pub official_name: Option<Option<String>>,
    pub subtype: Option<Option<String>>,
    pub account_group: Option<String>,
    pub import_pending: Option<bool>,
    pub apr_pct: Option<Option<f64>>,
    pub min_payment_cents: Option<Option<i64>>,
    pub payoff_date: Option<Option<String>>,
    pub limit_cents: Option<Option<i64>>,
    pub original_balance_cents: Option<Option<i64>>,
    pub started_at: Option<Option<String>>,
}

#[derive(Debug, Clone, Deserialize, Type)]
pub struct NewAccount {
    pub owner: String,
    pub bank: String,
    pub r#type: AccountType,
    pub name: String,
    pub last4: Option<String>,
    pub currency: String,
    pub color: String,
    pub opening_balance_cents: i64,
    #[serde(default = "default_source")]
    pub source: String,
    #[serde(default = "default_liquidity_type")]
    pub liquidity_type: String,
    #[serde(default = "default_emergency_fund_eligible")]
    pub emergency_fund_eligible: bool,
    pub goal_earmark: Option<String>,
    pub apy_pct: Option<f64>,
    pub simplefin_account_id: Option<String>,
    pub nickname: Option<String>,
    pub connection_id: Option<String>,
    pub institution_id: Option<String>,
    pub external_account_id: Option<String>,
    pub official_name: Option<String>,
    pub mask: Option<String>,
    pub subtype: Option<String>,
    #[serde(default = "default_account_group")]
    pub account_group: String,
    pub available_balance_cents: Option<i64>,
    pub balance_date: Option<DateTime<Utc>>,
    pub extra_json: Option<String>,
    pub raw_json: Option<String>,
    #[serde(default = "default_import_pending")]
    pub import_pending: bool,
    pub apr_pct: Option<f64>,
    pub min_payment_cents: Option<i64>,
    pub payoff_date: Option<String>,
    pub limit_cents: Option<i64>,
    pub original_balance_cents: Option<i64>,
    pub started_at: Option<String>,
}
