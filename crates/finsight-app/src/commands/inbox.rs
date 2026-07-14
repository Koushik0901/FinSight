use crate::{
    error::{AppError, AppResult},
    AppState,
};
use chrono::{Datelike, Duration, NaiveDate, Utc};
use finsight_agent::LOW_CONFIDENCE_THRESHOLD;
use finsight_core::repos::run;
use rusqlite::params;
use serde::Serialize;
use specta::Type;

/// A single prioritized action item in the Financial Inbox.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ActionItem {
    /// Stable ID for this item — used as React key and for deduplication.
    pub id: String,
    /// "review" | "bills" | "budget" | "goals" | "savings"
    pub category: String,
    /// "high" | "medium" | "low"
    pub priority: String,
    pub title: String,
    pub detail: String,
    pub action_label: String,
    /// Frontend route the CTA navigates to (e.g., "/transactions").
    pub action_route: String,
    /// Optional count for badge display (e.g., number of uncategorized txns).
    pub badge_count: Option<i64>,
    /// Optional monetary amount in cents (e.g., bill amount).
    pub amount_cents: Option<i64>,
}

fn fmt_money(cents: i64) -> String {
    let abs = cents.abs() as f64 / 100.0;
    format!("${abs:.0}")
}

#[tauri::command]
#[specta::specta]
pub async fn get_action_items(state: tauri::State<'_, AppState>) -> AppResult<Vec<ActionItem>> {
    let db = (*state.db).clone();

    run(&db, move |conn| {
        let now = Utc::now();
        let today = now.format("%Y-%m-%d").to_string();
        let month_start = now.format("%Y-%m-01").to_string();
        let week_out = (now + Duration::days(7)).format("%Y-%m-%d").to_string();

        let mut items: Vec<ActionItem> = Vec::new();

        // ── 1. Uncategorized expense transactions ─────────────────────────────
        // Exclude transfers: the categorizer never assigns them a category, so a
        // negative transfer (e.g. an internal "Internet Withdrawal") would keep
        // this action item permanently non-clearable.
        let uncategorized_count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM transactions t \
                     WHERE category_id IS NULL AND amount_cents < 0 AND is_transfer = 0 AND {}",
                    finsight_core::metrics::non_investment_txn_predicate("t")
                ),
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if uncategorized_count > 0 {
            items.push(ActionItem {
                id: "uncategorized-transactions".to_string(),
                category: "review".to_string(),
                priority: if uncategorized_count >= 10 {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                title: format!(
                    "{uncategorized_count} transaction{} need categorizing",
                    if uncategorized_count == 1 { "" } else { "s" }
                ),
                detail: "Uncategorized transactions make your budget reports unreliable. \
                         A few minutes now keeps your data clean."
                    .to_string(),
                action_label: "Review transactions".to_string(),
                action_route: "/transactions?filter=no_category".to_string(),
                badge_count: Some(uncategorized_count),
                amount_cents: None,
            });
        }

        // ── 1b. Transfer-like transactions with no verdict ────────────────────
        // Rows that carry transfer vocabulary but were neither flagged nor
        // paired: bare "INTERNET TRANSFER <ref>" legs whose counter-leg was
        // never imported, person-to-person e-transfers that may be rent or
        // reimbursements. Until the user rules on them they silently count as
        // income/expense, so they get a first-class review surface.
        let review_predicate = finsight_core::categorize::transfer_review_predicate("transactions");
        let (transfer_review_count, transfer_review_total): (i64, i64) = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*), COALESCE(SUM(ABS(amount_cents)), 0) \
                     FROM transactions WHERE {review_predicate}"
                ),
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0));

        if transfer_review_count > 0 {
            items.push(ActionItem {
                id: "transfer-review".to_string(),
                category: "review".to_string(),
                priority: if transfer_review_count >= 5 {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                title: format!(
                    "{transfer_review_count} transaction{} look{} like transfers — {}",
                    if transfer_review_count == 1 { "" } else { "s" },
                    if transfer_review_count == 1 { "s" } else { "" },
                    fmt_money(transfer_review_total)
                ),
                detail: "These carry transfer wording but have no matching leg in your \
                         accounts. If one is a move between your own accounts, mark it a \
                         transfer so it stops counting as income or spending; if it's real \
                         (rent, a gift, a reimbursement), categorize it."
                    .to_string(),
                action_label: "Review transfers".to_string(),
                action_route: "/transactions?filter=transfer_review".to_string(),
                badge_count: Some(transfer_review_count),
                amount_cents: Some(transfer_review_total),
            });
        }

        // ── 2. Anomalies flagged by the agent ────────────────────────────────
        let anomaly_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE is_anomaly = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if anomaly_count > 0 {
            items.push(ActionItem {
                id: "anomalies-flagged".to_string(),
                category: "review".to_string(),
                priority: "high".to_string(),
                title: format!(
                    "{anomaly_count} unusual transaction{} flagged",
                    if anomaly_count == 1 { "" } else { "s" }
                ),
                detail: "The agent spotted spending that looks out of the ordinary. \
                         Review to confirm or dismiss — could be fraud or an honest mistake."
                    .to_string(),
                action_label: "Review anomalies".to_string(),
                action_route: "/transactions?filter=anomalies".to_string(),
                badge_count: Some(anomaly_count),
                amount_cents: None,
            });
        }

        // ── 3. Low-confidence AI categorizations ─────────────────────────────
        let low_confidence_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions \
                 WHERE ai_confidence IS NOT NULL AND ai_confidence < ?1 \
                   AND (SELECT source FROM categorizations c \
                        WHERE c.txn_id = transactions.id ORDER BY c.at DESC LIMIT 1) = 'llm'",
                params![LOW_CONFIDENCE_THRESHOLD],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if low_confidence_count > 0 {
            items.push(ActionItem {
                id: "low-confidence-categorizations".to_string(),
                category: "review".to_string(),
                priority: if low_confidence_count >= 5 { "medium".to_string() } else { "low".to_string() },
                title: format!(
                    "{low_confidence_count} categor{} flagged as uncertain by the AI",
                    if low_confidence_count == 1 { "y" } else { "ies" }
                ),
                detail: "The AI assigned these categories with low confidence. \
                         Run a re-check to apply any rules you've added since the last import, \
                         or correct them manually in your transactions."
                    .to_string(),
                action_label: "Review in transactions".to_string(),
                action_route: "/transactions?filter=needs_review".to_string(),
                badge_count: Some(low_confidence_count),
                amount_cents: None,
            });
        }

        // ── 4. Reimbursable transactions pending ─────────────────────────────
        let reimbursable_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE is_reimbursable = 1",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);

        if reimbursable_count > 0 {
            let reimbursable_total: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(ABS(amount_cents)), 0) FROM transactions WHERE is_reimbursable = 1",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            items.push(ActionItem {
                id: "reimbursable-pending".to_string(),
                category: "review".to_string(),
                priority: "medium".to_string(),
                title: format!(
                    "{reimbursable_count} reimbursable expense{} — {}",
                    if reimbursable_count == 1 { "" } else { "s" },
                    fmt_money(reimbursable_total)
                ),
                detail: "You've marked expenses as reimbursable. \
                         Follow up to make sure you get your money back."
                    .to_string(),
                action_label: "View in transactions".to_string(),
                action_route: "/transactions?filter=needs_review".to_string(),
                badge_count: Some(reimbursable_count),
                amount_cents: Some(reimbursable_total),
            });
        }

        // ── 4. Bills due within 7 days ────────────────────────────────────────
        // Reuse the same recurring detection logic but only surface items due soon.
        let cutoff_past = (now - Duration::days(395)).format("%Y-%m-%d").to_string();
        let mut bill_stmt = conn
            .prepare(&format!(
                "WITH dated AS (
                   SELECT t.merchant_raw,
                          date(t.posted_at) AS d,
                          t.amount_cents,
                          LAG(date(t.posted_at)) OVER (
                            PARTITION BY t.merchant_raw ORDER BY t.posted_at
                          ) AS prev_d
                   FROM transactions t
                   WHERE t.posted_at >= ?1 AND t.amount_cents < 0 AND {}
                 ),
                 gaps AS (
                   SELECT merchant_raw, d, amount_cents,
                          julianday(d) - julianday(prev_d) AS gap
                   FROM dated WHERE prev_d IS NOT NULL
                 ),
                 agg AS (
                   SELECT merchant_raw,
                          AVG(gap) AS avg_gap,
                          MAX(d) AS last_seen,
                          MAX(amount_cents) AS last_amount
                   FROM gaps
                   WHERE gap BETWEEN 5 AND 400
                   GROUP BY merchant_raw
                   HAVING COUNT(*) >= 2 AND AVG(gap) < 400
                 )
                 SELECT merchant_raw, avg_gap, last_seen, last_amount
                 FROM agg
                 ORDER BY last_amount ASC",
                finsight_core::metrics::non_investment_txn_predicate("t")
            ))
            .ok();

        if let Some(ref mut stmt) = bill_stmt {
            let rows = stmt
                .query_map(params![cutoff_past], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, f64>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                })
                .ok();

            let mut due_soon: Vec<(String, String, i64)> = Vec::new(); // (merchant, next_date, amount)

            if let Some(rows) = rows {
                for row in rows.flatten() {
                    let (merchant, avg_gap, last_seen, last_amount) = row;
                    if let Ok(d) = NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d") {
                        let next = d + Duration::days(avg_gap.round() as i64);
                        let next_str = next.format("%Y-%m-%d").to_string();
                        if next_str > today && next_str <= week_out {
                            due_soon.push((merchant, next_str, last_amount));
                        }
                    }
                }
            }

            for (merchant, next_date, amount) in &due_soon {
                let days_away = NaiveDate::parse_from_str(next_date, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| {
                        NaiveDate::parse_from_str(&today, "%Y-%m-%d")
                            .ok()
                            .map(|t| (d - t).num_days())
                    })
                    .unwrap_or(0);

                let when = if days_away == 0 {
                    "today".to_string()
                } else if days_away == 1 {
                    "tomorrow".to_string()
                } else {
                    format!("in {days_away} days")
                };

                items.push(ActionItem {
                    id: format!("bill-due-{}", merchant.to_lowercase().replace(' ', "-")),
                    category: "bills".to_string(),
                    priority: if days_away <= 2 { "high".to_string() } else { "medium".to_string() },
                    title: format!("{merchant} due {when}"),
                    detail: format!(
                        "Expected charge of {}. Make sure your account has sufficient funds.",
                        fmt_money(amount.abs())
                    ),
                    action_label: "View recurring".to_string(),
                    action_route: "/recurring".to_string(),
                    badge_count: None,
                    amount_cents: Some(*amount),
                });
            }
        }

        // ── 5. Budget envelopes over limit this month ────────────────────────
        let mut over_stmt = conn
            .prepare(
                "SELECT c.label, e.budget_cents,
                        COALESCE(SUM(CASE WHEN t.amount_cents < 0 THEN -t.amount_cents ELSE 0 END), 0) AS spent
                 FROM budget_envelopes e
                 JOIN categories c ON c.id = e.category_id
                 LEFT JOIN transactions t ON t.category_id = e.category_id
                                        AND t.posted_at >= ?1
                 WHERE e.month = ?2
                 GROUP BY e.id, c.label, e.budget_cents
                 HAVING spent > e.budget_cents
                 ORDER BY (spent - e.budget_cents) DESC
                 LIMIT 5",
            )
            .ok();

        if let Some(ref mut stmt) = over_stmt {
            let month = now.format("%Y-%m").to_string();
            let rows = stmt
                .query_map(params![month_start, month], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, i64>(1)?,
                        r.get::<_, i64>(2)?,
                    ))
                })
                .ok();

            if let Some(rows) = rows {
                for row in rows.flatten() {
                    let (label, budget_cents, spent_cents) = row;
                    let over_by = spent_cents - budget_cents;
                    items.push(ActionItem {
                        id: format!(
                            "budget-over-{}",
                            label.to_lowercase().replace(' ', "-")
                        ),
                        category: "budget".to_string(),
                        priority: if over_by > budget_cents / 2 {
                            "high".to_string()
                        } else {
                            "medium".to_string()
                        },
                        title: format!("{label} is over budget by {}", fmt_money(over_by)),
                        detail: format!(
                            "You budgeted {} but spent {} this month. \
                             Adjust next month's plan or find cuts now.",
                            fmt_money(budget_cents),
                            fmt_money(spent_cents)
                        ),
                        action_label: "Review budget".to_string(),
                        action_route: "/budget".to_string(),
                        badge_count: None,
                        amount_cents: Some(over_by),
                    });
                }
            }
        }

        // ── 6. Goals off track ────────────────────────────────────────────────
        let mut goal_stmt = conn
            .prepare(
                "SELECT id, name, target_cents, current_cents, monthly_cents, target_date
                 FROM goals WHERE archived_at IS NULL AND target_date IS NOT NULL",
            )
            .ok();

        if let Some(ref mut stmt) = goal_stmt {
            let today_naive = NaiveDate::parse_from_str(&today, "%Y-%m-%d").ok();

            let rows = stmt
                .query_map([], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, String>(5)?,
                    ))
                })
                .ok();

            if let Some(rows) = rows {
                for row in rows.flatten() {
                    let (id, name, target_cents, current_cents, monthly_cents, target_date) = row;
                    let remaining = (target_cents - current_cents).max(0);
                    if remaining == 0 || monthly_cents <= 0 {
                        continue;
                    }

                    let months_needed =
                        (remaining + monthly_cents - 1) / monthly_cents;

                    if let (Some(today_d), Ok(target_d)) = (
                        today_naive,
                        NaiveDate::parse_from_str(&target_date, "%Y-%m-%d"),
                    ) {
                        let months_left = {
                            let years = (target_d.year() - today_d.year()) as i64;
                            let months = (target_d.month() as i64) - (today_d.month() as i64);
                            years * 12 + months
                        };

                        if months_needed > months_left + 1 {
                            let gap_months = months_needed - months_left;
                            let needed_extra = if months_left > 0 {
                                remaining / months_left - monthly_cents
                            } else {
                                remaining - monthly_cents
                            };

                            items.push(ActionItem {
                                id: format!("goal-off-track-{id}"),
                                category: "goals".to_string(),
                                priority: if gap_months > 6 {
                                    "high".to_string()
                                } else {
                                    "medium".to_string()
                                },
                                title: format!("{name} is behind schedule"),
                                detail: format!(
                                    "At your current pace you'll reach this goal {gap_months} \
                                     month{} after your target date. \
                                     Adding {} more per month would get you back on track.",
                                    if gap_months == 1 { "" } else { "s" },
                                    fmt_money(needed_extra.max(0))
                                ),
                                action_label: "Adjust goal".to_string(),
                                action_route: "/goals".to_string(),
                                badge_count: None,
                                amount_cents: None,
                            });
                        }
                    }
                }
            }
        }

        // ── 7. Savings rate below 10% (last 90 days) ─────────────────────────
        // Through the metrics layer, not hand-rolled SQL: this item used to
        // re-derive the rate WITHOUT the transfer exclusion, so moving money
        // between your own accounts changed the "savings rate" the Inbox nagged
        // about while every other screen disagreed.
        let rolling = finsight_core::metrics::rolling_averages(conn, 90).unwrap_or_default();

        if rolling.avg_monthly_income_cents > 0 {
            let savings_rate_pct = rolling.savings_rate_pct;

            if savings_rate_pct < 10 {
                items.push(ActionItem {
                    id: "savings-rate-low".to_string(),
                    category: "savings".to_string(),
                    priority: if savings_rate_pct < 0 {
                        "high".to_string()
                    } else {
                        "medium".to_string()
                    },
                    title: format!(
                        "Savings rate is {}% — below the 10% minimum",
                        savings_rate_pct.max(0)
                    ),
                    detail: "The Richest Man in Babylon's first rule: pay yourself first — \
                             keep at least 10 cents of every dollar earned. \
                             Even small cuts add up quickly."
                        .to_string(),
                    action_label: "Plan with Copilot".to_string(),
                    action_route: "/copilot".to_string(),
                    badge_count: None,
                    amount_cents: None,
                });
            }
        }

        // ── 8. Missing emergency fund (< 1 month of expenses) ────────────────
        let avg_monthly_expense: i64 = rolling.avg_monthly_expense_cents;

        if avg_monthly_expense > 0 {
            // Look for any emergency-fund-type goal with meaningful balance
            let emergency_cents: i64 = conn
                .query_row(
                    "SELECT COALESCE(SUM(current_cents), 0)
                     FROM goals
                     WHERE archived_at IS NULL
                       AND (LOWER(name) LIKE '%emergency%' OR type = 'build-balance')",
                    [],
                    |r| r.get(0),
                )
                .unwrap_or(0);

            let months_covered =
                if avg_monthly_expense > 0 { emergency_cents / avg_monthly_expense } else { 0 };

            if months_covered < 1 {
                items.push(ActionItem {
                    id: "emergency-fund-missing".to_string(),
                    category: "savings".to_string(),
                    priority: "medium".to_string(),
                    title: "Emergency fund covers less than 1 month of expenses".to_string(),
                    detail: format!(
                        "Dave Ramsey's Baby Step 1: save a starter emergency fund of $1,000, \
                         then build to 3–6 months of expenses ({}–{}). \
                         Without it, any unexpected bill becomes debt.",
                        fmt_money(avg_monthly_expense * 3),
                        fmt_money(avg_monthly_expense * 6)
                    ),
                    action_label: "Set up emergency fund".to_string(),
                    action_route: "/goals".to_string(),
                    badge_count: None,
                    amount_cents: None,
                });
            }
        }

        // ── Sort: high first, then medium, then low ───────────────────────────
        items.sort_by_key(|item| match item.priority.as_str() {
            "high" => 0,
            "medium" => 1,
            _ => 2,
        });

        Ok(items)
    })
    .await
    .map_err(AppError::from)
}
