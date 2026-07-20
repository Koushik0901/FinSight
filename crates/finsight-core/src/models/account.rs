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
    /// ISO date a promotional rate ends. `None` means `apr_pct` is permanent.
    /// See [`effective_apr_pct`] — `apr_pct` always means "the rate right now",
    /// which is why adding a promo needs no rewrite of existing data.
    pub promo_apr_expires_on: Option<String>,
    /// The rate that takes over once the promo ends. `None` alongside a set
    /// expiry means the user recorded that the rate WILL change but not to
    /// what — a real state that must be surfaced as unknown, never silently
    /// projected as 0%.
    pub post_promo_apr_pct: Option<f64>,
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
    // Deliberately NOT `#[serde(default)]`, unlike the computed fields above:
    // these are stored columns the query always supplies, and making them
    // optional here would generate a TS type subtly different from `Account`'s,
    // so an `AccountSummary` would stop being usable everywhere an `Account` is.
    pub promo_apr_expires_on: Option<String>,
    pub post_promo_apr_pct: Option<f64>,
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
    pub promo_apr_expires_on: Option<Option<String>>,
    pub post_promo_apr_pct: Option<Option<f64>>,
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
    #[serde(default)]
    pub promo_apr_expires_on: Option<String>,
    #[serde(default)]
    pub post_promo_apr_pct: Option<f64>,
}

/// What APR applies to a debt on a given date, and whether we actually know.
///
/// `apr_pct` on an account always means "the rate in effect right now". A
/// promotional period says that rate has an end date and something else takes
/// over. Ranking and payoff projections care about the FORWARD rate, because a
/// 0% balance about to become 22.99% is not a 0% balance for planning purposes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EffectiveApr {
    /// The rate that applies on the asked-about date.
    Known(f64),
    /// A promo is recorded as ending, but no post-promo rate was given. The
    /// honest answer is "it changes, to something we were not told" — never a
    /// silent 0%, which would rank this debt as free money to ignore.
    UnknownAfterPromo,
    /// No APR recorded at all.
    Unknown,
}

impl EffectiveApr {
    /// The rate as a number, or `None` when we do not have one. Callers that
    /// must sort still need a value; they should treat `None` explicitly rather
    /// than defaulting it to zero.
    pub fn pct(self) -> Option<f64> {
        match self {
            EffectiveApr::Known(v) => Some(v),
            _ => None,
        }
    }
}

/// Which APR governs on `on_date` (ISO `YYYY-MM-DD`).
///
/// Before the expiry — or with no promo recorded — that is `apr_pct`. On or
/// after it, `post_promo_apr_pct`. A malformed or unparseable expiry is treated
/// as no promo rather than panicking or guessing: dates can arrive from imports
/// and a bad one must not make the app claim a rate change it cannot place.
pub fn effective_apr_pct(
    apr_pct: Option<f64>,
    promo_apr_expires_on: Option<&str>,
    post_promo_apr_pct: Option<f64>,
    on_date: chrono::NaiveDate,
) -> EffectiveApr {
    let expiry = promo_apr_expires_on
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());

    match expiry {
        // Promo still running on this date, or no promo at all.
        Some(end) if on_date < end => apr_pct.map_or(EffectiveApr::Unknown, EffectiveApr::Known),
        None => apr_pct.map_or(EffectiveApr::Unknown, EffectiveApr::Known),
        // Promo has ended by this date.
        Some(_) => match post_promo_apr_pct {
            Some(v) => EffectiveApr::Known(v),
            None => EffectiveApr::UnknownAfterPromo,
        },
    }
}

/// A promotional rate close enough to expiry to be worth telling the user
/// about, with what they need in order to act.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PromoExpiryWarning {
    pub days_left: i64,
    /// Positive amount owed that will start accruing the new rate.
    pub owed_cents: i64,
    /// The rate it becomes. `None` when the user recorded an end date but not
    /// what it reverts to — still worth warning about, and the copy should say
    /// what is missing rather than invent a number.
    pub becomes_apr_pct: Option<f64>,
}

/// Whether a debt's promotional rate warrants a warning on `today`.
///
/// Deliberately returns `None` — silence — in every case where a warning would
/// be noise rather than news. An alert that fires when nothing is wrong is
/// worse than no alert, because it teaches people to dismiss the ones that
/// matter:
///
/// * no promotional period recorded, or a date we cannot parse
/// * the expiry has already passed — the rate is simply what it is now, and
///   there is no window left to act in
/// * further out than `lead_days`, where there is nothing to do yet
/// * nothing owed, so no interest is at stake either way
/// * the rate is not getting worse
pub fn promo_expiry_warning(
    apr_pct: Option<f64>,
    promo_apr_expires_on: Option<&str>,
    post_promo_apr_pct: Option<f64>,
    balance_cents: i64,
    today: chrono::NaiveDate,
    lead_days: i64,
) -> Option<PromoExpiryWarning> {
    // Debt is stored as a negative balance.
    let owed_cents = -balance_cents;
    if owed_cents <= 0 {
        return None;
    }
    let end = promo_apr_expires_on
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())?;

    let days_left = (end - today).num_days();
    if !(0..=lead_days).contains(&days_left) {
        return None;
    }
    if let (Some(now_rate), Some(after)) = (apr_pct, post_promo_apr_pct) {
        if after <= now_rate {
            return None;
        }
    }
    Some(PromoExpiryWarning {
        days_left,
        owed_cents,
        becomes_apr_pct: post_promo_apr_pct,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn no_promo_means_the_rate_never_changes() {
        // The overwhelming majority of accounts, and every account that
        // existed before promos were modelled. Must be unaffected.
        for date in ["2020-01-01", "2026-07-19", "2099-12-31"] {
            assert_eq!(
                effective_apr_pct(Some(19.99), None, None, d(date)),
                EffectiveApr::Known(19.99)
            );
        }
        assert_eq!(
            effective_apr_pct(None, None, None, d("2026-07-19")),
            EffectiveApr::Unknown,
            "no APR recorded at all stays unknown, not zero"
        );
    }

    #[test]
    fn the_promotional_rate_applies_up_to_but_not_on_the_expiry_date() {
        let expiry = Some("2026-09-01");
        let promo = Some(0.0);
        let after = Some(22.99);

        assert_eq!(
            effective_apr_pct(promo, expiry, after, d("2026-08-31")),
            EffectiveApr::Known(0.0),
            "day before expiry: still promotional"
        );
        assert_eq!(
            effective_apr_pct(promo, expiry, after, d("2026-09-01")),
            EffectiveApr::Known(22.99),
            "ON the expiry date the new rate governs — a boundary that decides \
             whether someone is warned in time"
        );
        assert_eq!(
            effective_apr_pct(promo, expiry, after, d("2027-01-01")),
            EffectiveApr::Known(22.99)
        );
    }

    #[test]
    fn an_expiry_without_a_post_promo_rate_is_unknown_not_zero() {
        // The dangerous default. Projecting 0% forever would rank the debt as
        // free money and bury it at the bottom of a payoff plan.
        let got = effective_apr_pct(Some(0.0), Some("2026-09-01"), None, d("2026-10-01"));
        assert_eq!(got, EffectiveApr::UnknownAfterPromo);
        assert_eq!(got.pct(), None, "callers must not get a number to sort on");
    }

    #[test]
    fn a_malformed_expiry_degrades_to_todays_rate_instead_of_panicking() {
        // Dates can arrive from imports. A bad one must not make the app claim
        // a rate change it cannot place, and must not crash.
        for bad in ["", "   ", "not-a-date", "2026-13-45", "09/01/2026"] {
            assert_eq!(
                effective_apr_pct(Some(11.5), Some(bad), Some(22.99), d("2026-10-01")),
                EffectiveApr::Known(11.5),
                "unparseable expiry {bad:?} is treated as no promo"
            );
        }
    }

    // ── When to warn ────────────────────────────────────────────────────────

    const LEAD: i64 = 60;
    const OWED: i64 = -500_000; // $5,000 owed, stored negative

    fn warn_on(day: &str, expiry: Option<&str>, post: Option<f64>) -> Option<PromoExpiryWarning> {
        promo_expiry_warning(Some(0.0), expiry, post, OWED, d(day), LEAD)
    }

    #[test]
    fn warns_inside_the_lead_window_and_stays_quiet_outside_it() {
        let expiry = Some("2026-09-01");
        assert!(
            warn_on("2026-08-25", expiry, Some(22.99)).is_some(),
            "a week out: act now"
        );
        assert_eq!(
            warn_on("2026-09-01", expiry, Some(22.99)).map(|w| w.days_left),
            Some(0),
            "the day it lands is still the last chance to act"
        );
        assert!(
            warn_on("2026-06-01", expiry, Some(22.99)).is_none(),
            "92 days out is beyond the lead window — nothing to do yet"
        );
        assert!(
            warn_on("2026-09-02", expiry, Some(22.99)).is_none(),
            "after expiry there is no window left; the rate is simply what it is"
        );
    }

    #[test]
    fn stays_quiet_when_there_is_nothing_to_warn_about() {
        // No promo at all.
        assert!(warn_on("2026-08-25", None, Some(22.99)).is_none());
        // Unparseable date — say nothing rather than guess.
        assert!(warn_on("2026-08-25", Some("whenever"), Some(22.99)).is_none());
        // Nothing owed: no interest is at stake either way.
        assert!(
            promo_expiry_warning(Some(0.0), Some("2026-09-01"), Some(22.99), 0, d("2026-08-25"), LEAD)
                .is_none()
        );
        // A paid-off card in credit.
        assert!(
            promo_expiry_warning(Some(0.0), Some("2026-09-01"), Some(22.99), 5_000, d("2026-08-25"), LEAD)
                .is_none()
        );
        // The rate is not getting worse — firing here would train the user to
        // dismiss these.
        assert!(
            promo_expiry_warning(Some(15.0), Some("2026-09-01"), Some(9.0), OWED, d("2026-08-25"), LEAD)
                .is_none()
        );
        assert!(
            promo_expiry_warning(Some(15.0), Some("2026-09-01"), Some(15.0), OWED, d("2026-08-25"), LEAD)
                .is_none(),
            "an unchanged rate is not news"
        );
    }

    #[test]
    fn warns_even_when_the_new_rate_was_never_recorded() {
        // "It changes, to something you didn't tell me" is still worth saying —
        // arguably more so, because the user has to go find out.
        let w = warn_on("2026-08-25", Some("2026-09-01"), None).expect("still warns");
        assert_eq!(w.becomes_apr_pct, None);
        assert_eq!(w.owed_cents, 500_000, "reported as a positive amount owed");
    }

    #[test]
    fn a_promo_that_lowers_the_rate_is_still_modelled_honestly() {
        // Nothing says the post-promo rate must be worse. A teaser that ENDS
        // into a lower rate (rare, but real on some loans) must not be forced
        // into the "gets worse" shape.
        assert_eq!(
            effective_apr_pct(Some(15.0), Some("2026-09-01"), Some(9.0), d("2026-10-01")),
            EffectiveApr::Known(9.0)
        );
    }
}
