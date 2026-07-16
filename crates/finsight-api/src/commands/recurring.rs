use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::recurring::{detect_recurring, RecurringKind};
use finsight_core::repos::run;
use serde::Serialize;
use specta::Type;

/// A recurring transaction detected from transaction history (Phase 6 redesign).
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecurringItem {
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
    pub min_amount_cents: i64,
    pub max_amount_cents: i64,
    pub avg_gap_days: f64,
    pub occurrences: i64,
    pub last_seen: String,
    pub next_expected: String,
    pub cadence: String,
    /// True only for genuine subscriptions (not repeat purchases).
    pub is_subscription: bool,
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
