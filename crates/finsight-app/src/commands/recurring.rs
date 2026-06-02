use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::{Duration, NaiveDate, Utc};
use finsight_core::repos::run;
use serde::Serialize;
use specta::Type;

/// A recurring transaction detected from transaction history.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecurringItem {
    pub merchant_raw: String,
    pub category_label: String,
    pub category_color: String,
    /// Most recent amount (negative = expense, positive = income)
    pub last_amount_cents: i64,
    /// Average gap between occurrences in days
    pub avg_gap_days: f64,
    /// How many times this has appeared
    pub occurrences: i64,
    /// Most recent posted_at date (ISO)
    pub last_seen: String,
    /// Estimated next date (ISO), based on last_seen + avg_gap
    pub next_expected: String,
    /// "monthly" | "weekly" | "biweekly" | "annual" | "irregular"
    pub cadence: String,
    /// Whether this looks like a subscription (small, regular negative charge)
    pub is_subscription: bool,
}

fn cadence_label(avg_gap: f64) -> &'static str {
    if avg_gap < 10.0  { "weekly" }
    else if avg_gap < 20.0 { "biweekly" }
    else if avg_gap < 45.0 { "monthly" }
    else if avg_gap < 100.0 { "quarterly" }
    else { "annual" }
}

#[tauri::command]
#[specta::specta]
pub async fn list_recurring(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<RecurringItem>> {
    let db = (*state.db).clone();

    run(&db, |conn| {
        // Detect recurring by finding merchants that appear at consistent intervals.
        // We look back 13 months so annual charges are detectable.
        let cutoff = (Utc::now() - Duration::days(395)).format("%Y-%m-%d").to_string();

        let mut stmt = conn.prepare(
            "WITH dated AS (
               SELECT t.merchant_raw,
                      date(t.posted_at) AS d,
                      t.amount_cents,
                      c.label AS cat_label,
                      COALESCE(c.color, '') AS cat_color,
                      LAG(date(t.posted_at)) OVER (
                        PARTITION BY t.merchant_raw
                        ORDER BY t.posted_at
                      ) AS prev_d
               FROM transactions t
               LEFT JOIN categories c ON c.id = t.category_id
               WHERE t.posted_at >= ?1
             ),
             gaps AS (
               SELECT merchant_raw, d, amount_cents, cat_label, cat_color,
                      julianday(d) - julianday(prev_d) AS gap
               FROM dated
               WHERE prev_d IS NOT NULL
             ),
             agg AS (
               SELECT merchant_raw,
                      MAX(cat_label) AS cat_label,
                      MAX(cat_color) AS cat_color,
                      AVG(gap) AS avg_gap,
                      COUNT(*) AS occurrences,
                      MAX(d) AS last_seen,
                      MAX(amount_cents) AS last_amount
               FROM gaps
               WHERE gap BETWEEN 5 AND 400
               GROUP BY merchant_raw
               HAVING occurrences >= 2 AND AVG(gap) < 400
             )
             SELECT merchant_raw, cat_label, cat_color, avg_gap, occurrences, last_seen, last_amount
             FROM agg
             ORDER BY ABS(last_amount) DESC",
        )?;

        let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok((
                r.get::<_, String>(0)?,  // merchant_raw
                r.get::<_, String>(1)?,  // cat_label
                r.get::<_, String>(2)?,  // cat_color
                r.get::<_, f64>(3)?,     // avg_gap
                r.get::<_, i64>(4)?,     // occurrences
                r.get::<_, String>(5)?,  // last_seen
                r.get::<_, i64>(6)?,     // last_amount
            ))
        })?;

        let mut out = Vec::new();
        for row in rows {
            let (merchant_raw, cat_label, cat_color, avg_gap, occurrences, last_seen, last_amount) = row?;
            let cadence = cadence_label(avg_gap).to_string();

            // Estimate next date
            let next_expected = if let Ok(d) = NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d") {
                let next = d + Duration::days(avg_gap.round() as i64);
                next.format("%Y-%m-%d").to_string()
            } else {
                last_seen.clone()
            };

            // Subscription heuristic: small-to-mid expense, monthly-or-shorter
            let is_subscription = last_amount < 0
                && last_amount > -20_000  // under $200
                && avg_gap < 45.0;

            out.push(RecurringItem {
                merchant_raw,
                category_label: cat_label,
                category_color: cat_color,
                last_amount_cents: last_amount,
                avg_gap_days: avg_gap,
                occurrences,
                last_seen,
                next_expected,
                cadence,
                is_subscription,
            });
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
