use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::recurring::{detect_recurring, PriceChange, RecurringKind};
use finsight_core::repos::run;
use finsight_core::subscriptions::{self, SubscriptionVerdict};
use serde::Serialize;
use specta::Type;

/// A detected material change in a fixed-price recurring charge's amount (#58).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PriceChangeDto {
    pub from_cents: i64,
    pub to_cents: i64,
    /// Signed percent change vs the prior price.
    pub pct: f64,
    pub effective_date: String,
    pub currency: String,
}

impl From<&PriceChange> for PriceChangeDto {
    fn from(p: &PriceChange) -> Self {
        Self {
            from_cents: p.from_cents,
            to_cents: p.to_cents,
            pct: p.pct,
            effective_date: p.effective_date.clone(),
            currency: p.currency.clone(),
        }
    }
}

/// A recurring transaction detected from transaction history (Phase 6 redesign).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecurringItem {
    /// The canonical grouping key — the handle the confirm/dismiss verdict is
    /// keyed on (`set_subscription_verdict`).
    pub merchant_key: String,
    pub merchant_raw: String,
    pub category_label: String,
    pub category_color: String,
    /// "subscription" | "bill" | "income" — genuine recurring commitments only.
    pub kind: String,
    /// 0..1 confidence this is a genuine recurring item.
    pub confidence: f64,
    /// Human-readable evidence for the classification.
    pub reasons: Vec<String>,
    pub last_amount_cents: i64,
    /// Cost per month with cadence accounted for, positive. Computed in
    /// `finsight-core` so the UI does not re-derive the normalisation and
    /// disagree with the budget and Copilot figures built on the same rule.
    pub monthly_equivalent_cents: i64,
    pub min_amount_cents: i64,
    pub max_amount_cents: i64,
    pub avg_gap_days: f64,
    pub occurrences: i64,
    pub last_seen: String,
    pub next_expected: String,
    pub cadence: String,
    /// True only for genuine subscriptions (not repeat purchases).
    pub is_subscription: bool,
    /// Whether this item is confident enough to feed forward-looking
    /// projections. Low-confidence entries stay VISIBLE — the user is the one
    /// who can confirm or dismiss them — but do not silently move arithmetic.
    pub feeds_projections: bool,
    /// A detected material price change on this series, if any (#58).
    pub price_change: Option<PriceChangeDto>,
    /// The user's durable verdict on this series: "confirmed" | "dismissed" |
    /// "cancelled" | null. A dismissed series is suppressed from alerts; a
    /// cancelled one stops ongoing alerts but flags a charge after the cancel
    /// date (#75).
    pub verdict: Option<String>,
    /// If the user marked this a free trial, the date it converts (`YYYY-MM-DD`);
    /// a heads-up fires shortly before. `null` when not a trial (#75).
    pub trial_ends_at: Option<String>,
    /// If the user marked this cancelled, the cancel date (`YYYY-MM-DD`); a
    /// charge after it is surfaced as a surprise. `null` otherwise (#75).
    pub cancelled_at: Option<String>,
}

fn kind_str(kind: RecurringKind) -> &'static str {
    match kind {
        RecurringKind::Subscription => "subscription",
        RecurringKind::Bill => "bill",
        RecurringKind::Income => "income",
        RecurringKind::Transfer => "transfer",
        RecurringKind::RepeatPurchase => "repeat_purchase",
    }
}

pub async fn list_recurring(state: &ApiState) -> AppResult<Vec<RecurringItem>> {
    let db = (*state.db).clone();

    run(&db, |conn| {
        // 13-month window so annual charges are detectable; the detector anchors
        // on the most recent transaction so historical imports still work.
        let items = detect_recurring(conn, 395)?;
        // The user's lifecycle overrides (verdict + trial/cancel), by merchant key.
        let overrides = subscriptions::load_overrides(conn)?;
        Ok(items
            .into_iter()
            // Show genuine recurring commitments + income only. Repeat purchases
            // (groceries/dining/ride-hailing) and internal transfers/card
            // payments are deliberately excluded from the recurring view.
            .filter(|i| {
                matches!(
                    i.kind,
                    RecurringKind::Subscription | RecurringKind::Bill | RecurringKind::Income
                )
            })
            .map(|i| RecurringItem {
                monthly_equivalent_cents: i.monthly_equivalent_cents(),
                feeds_projections: i.is_projection_obligation(),
                price_change: i.price_change.as_ref().map(PriceChangeDto::from),
                verdict: overrides.get(&i.merchant_key).map(|o| o.verdict.as_str().to_string()),
                trial_ends_at: overrides.get(&i.merchant_key).and_then(|o| o.trial_ends_at.clone()),
                cancelled_at: overrides.get(&i.merchant_key).and_then(|o| o.cancelled_at.clone()),
                merchant_key: i.merchant_key,
                merchant_raw: i.display_merchant,
                category_label: i.category_label.unwrap_or_default(),
                category_color: i.category_color.unwrap_or_default(),
                kind: kind_str(i.kind).to_string(),
                confidence: i.confidence,
                reasons: i.reasons,
                last_amount_cents: i.last_amount_cents,
                min_amount_cents: i.min_amount_cents,
                max_amount_cents: i.max_amount_cents,
                avg_gap_days: i.avg_gap_days,
                occurrences: i.occurrences,
                next_expected: i.next_expected.clone().unwrap_or_else(|| i.last_seen.clone()),
                last_seen: i.last_seen,
                cadence: i.cadence,
                is_subscription: i.kind == RecurringKind::Subscription,
            })
            .collect())
    })
    .await
    .map_err(AppError::from)
}

/// Set (or clear, with `verdict = None`) the user's confirm/dismiss verdict on a
/// detected subscription. `verdict` accepts "confirmed" | "dismissed"; any other
/// value (or null) clears it, so a bad string can't corrupt state.
pub async fn set_subscription_verdict(
    state: &ApiState,
    merchant_key: String,
    verdict: Option<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    let parsed = verdict.as_deref().and_then(SubscriptionVerdict::from_str);
    run(&db, move |conn| subscriptions::set_verdict(conn, &merchant_key, parsed))
        .await
        .map_err(AppError::from)
}

/// Mark a detected subscription as a free TRIAL converting on `trial_ends_at`
/// (`YYYY-MM-DD`), or clear it with `trial_ends_at = None`. `label` is the
/// display name captured for the reminder. A heads-up fires shortly before the
/// date (#75).
pub async fn set_subscription_trial(
    state: &ApiState,
    merchant_key: String,
    label: String,
    trial_ends_at: Option<String>,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        subscriptions::set_subscription_trial(conn, &merchant_key, &label, trial_ends_at.as_deref())
    })
    .await
    .map_err(AppError::from)
}

/// Mark a detected subscription CANCELLED as of `cancelled_at` (`YYYY-MM-DD`).
/// Ongoing price/renewal alerts stop; a charge dated after the cancel date is
/// surfaced as a surprise. `label` names the service in that alert (#75).
pub async fn mark_subscription_cancelled(
    state: &ApiState,
    merchant_key: String,
    label: String,
    cancelled_at: String,
) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        subscriptions::mark_subscription_cancelled(conn, &merchant_key, &label, &cancelled_at)
    })
    .await
    .map_err(AppError::from)
}
