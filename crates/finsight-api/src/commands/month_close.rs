//! Guided month-end financial close (issue #59).
//!
//! A close **surfaces and records** — it never rebuilds the resolvers the user
//! already has. Each data-quality flag summarizes a signal from the Inbox's
//! [`crate::commands::inbox::get_action_items`] aggregator and links to the
//! screen that fixes it; the money review shows the same `finsight-core::metrics`
//! numbers every screen shows. The one thing the close owns is **durability**:
//! completing it FREEZES the computed snapshot as stored JSON, so a later edit to
//! a historical transaction can never silently rewrite what the month recorded —
//! and the drift view recomputes today's numbers next to the frozen ones.
//!
//! Lifecycle is just `status ∈ {in_progress, completed, skipped}`. The persisted
//! `in_progress` row IS the pause; resuming reopens the screen. Reopening a
//! completed close keeps the frozen record; re-completing is an explicit,
//! never silent, re-record.

use crate::commands::inbox::{self, ActionItem};
use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::repos::run;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// A metric drifts from what was recorded once it moves this far (relative),
/// matching the scenario-staleness threshold so "materially changed" means one
/// thing across the app.
const DRIFT_THRESHOLD_PCT: f64 = 10.0;

/// Recurring-detection window (13 months — matches list_recurring).
const RECURRING_WINDOW_DAYS: i64 = 395;

const MONTH_NAMES: [&str; 13] = [
    "", "January", "February", "March", "April", "May", "June", "July", "August",
    "September", "October", "November", "December",
];

fn month_label(year: i32, month: i32) -> String {
    format!("{} {}", MONTH_NAMES.get(month as usize).copied().unwrap_or(""), year)
}

/// The month's key figures. Enriched over the old review snapshot with net
/// worth, debt, and a count of subscription price changes. Free-form goal JSON
/// is kept for the UI's progress list.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonthCloseSnapshot {
    pub income_cents: i64,
    pub expense_cents: i64,
    pub savings_cents: i64,
    pub savings_rate_pct: i64,
    pub net_worth_cents: i64,
    pub debt_total_cents: i64,
    pub over_budget_categories: Vec<String>,
    pub goal_progress: Vec<serde_json::Value>,
    pub subscription_change_count: i64,
}

/// The compact baseline drift is measured against — stored at freeze time so a
/// completed close can show "recorded then vs recomputed now" without mutating
/// the recorded snapshot (the V055 scenario-durability pattern).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct CloseBaseline {
    income_cents: i64,
    expense_cents: i64,
    net_worth_cents: i64,
}

/// A data-quality or subscription-change item to review at close. Mirrors an
/// Inbox action item (or #58 subscription changes) and carries a route to the
/// screen that resolves it — the close never resolves it inline.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CloseFlag {
    pub id: String,
    pub category: String,
    pub priority: String,
    pub title: String,
    pub detail: String,
    pub action_route: String,
    pub count: Option<i64>,
    /// Whether the user acknowledged this at close (frozen with the record).
    pub acknowledged: bool,
}

/// One "recorded then vs recomputed now" line for a completed close.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DriftLine {
    pub label: String,
    pub recorded_cents: i64,
    pub current_cents: i64,
    pub changed_materially: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthCloseView {
    pub year: i32,
    pub month: i32,
    pub month_label: String,
    /// "not_started" | "in_progress" | "completed" | "skipped".
    pub status: String,
    pub notes: Option<String>,
    pub completed_at: Option<String>,
    /// Live figures while in progress; the FROZEN figures once completed.
    pub snapshot: MonthCloseSnapshot,
    /// Live flags while in progress; the frozen (acknowledged-marked) set once completed.
    pub flags: Vec<CloseFlag>,
    /// Only populated for a completed close whose recorded figures now differ.
    pub drift: Vec<DriftLine>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthCloseListItem {
    pub year: i32,
    pub month: i32,
    pub month_label: String,
    pub status: String,
    pub completed_at: Option<String>,
    pub savings_rate_pct: i64,
    pub net_worth_cents: i64,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveMonthCloseInput {
    pub year: i32,
    pub month: i32,
    /// Target status: "in_progress" (start/resume), "completed" (freeze), "skipped".
    pub status: String,
    pub notes: Option<String>,
    /// Flag ids the user ticked as acknowledged — recorded when completing.
    #[serde(default)]
    pub acknowledged_flag_ids: Vec<String>,
}

fn validate_month(month: i32) -> Result<(), finsight_core::CoreError> {
    if (1..=12).contains(&month) {
        Ok(())
    } else {
        Err(finsight_core::CoreError::InvalidState("month must be between 1 and 12".into()))
    }
}

/// Compute the month's snapshot from live data. Degrades safely: a sparse month
/// with no budgets, goals, or debt yields zeros and empty lists, never an error.
fn compute_snapshot(
    conn: &mut rusqlite::Connection,
    year: i32,
    month: i32,
) -> Result<(MonthCloseSnapshot, CloseBaseline), finsight_core::CoreError> {
    let month_start = format!("{year}-{month:02}-01");
    let month_end = format!(
        "{}-{:02}-01",
        if month == 12 { year + 1 } else { year },
        if month == 12 { 1 } else { month + 1 }
    );
    let month_str = format!("{year}-{month:02}");

    let (income_cents, expense_cents) =
        finsight_core::metrics::income_expense_between(conn, &month_start, &month_end)?;
    let savings_cents = income_cents - expense_cents;
    let savings_rate_pct = finsight_core::metrics::savings_rate_pct(income_cents, expense_cents);

    let balances = finsight_core::metrics::balance_breakdown(conn)?;

    let over_budget_categories: Vec<String> = {
        let mut stmt = conn.prepare(
            "WITH actuals AS (
               SELECT category_id, SUM(CASE WHEN settle_up = 1 THEN -amount_cents
                                            WHEN amount_cents < 0 THEN -amount_cents
                                            ELSE 0 END) AS spent
               FROM transactions
               WHERE posted_at >= ?1 AND posted_at < ?2 AND (amount_cents < 0 OR settle_up = 1) AND is_transfer = 0
               GROUP BY category_id
             )
             SELECT c.label FROM budgets b
             JOIN categories c ON c.id = b.category_id
             JOIN actuals a ON a.category_id = b.category_id
             WHERE b.month = ?3 AND a.spent > b.amount_cents",
        )?;
        let rows = stmt
            .query_map(rusqlite::params![&month_start, &month_end, &month_str], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, rusqlite::Error>>()?;
        rows
    };

    let goal_progress = finsight_core::repos::goals::list(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|goal| {
            serde_json::json!({
                "id": goal.id,
                "name": goal.name,
                "currentCents": goal.current_cents,
                "targetCents": goal.target_cents,
                "pctComplete": if goal.target_cents > 0 {
                    ((goal.current_cents * 100) / goal.target_cents).clamp(0, 100)
                } else { 0 }
            })
        })
        .collect();

    // Subscription price changes whose new price took effect this month (#58).
    let subscription_change_count = finsight_core::recurring::detect_recurring(conn, RECURRING_WINDOW_DAYS)
        .unwrap_or_default()
        .iter()
        .filter(|i| i.price_change.as_ref().is_some_and(|pc| pc.effective_date.starts_with(&month_str)))
        .count() as i64;

    let snapshot = MonthCloseSnapshot {
        income_cents,
        expense_cents,
        savings_cents,
        savings_rate_pct,
        net_worth_cents: balances.net_worth_cents,
        debt_total_cents: balances.debt_cents,
        over_budget_categories,
        goal_progress,
        subscription_change_count,
    };
    let baseline = CloseBaseline {
        income_cents,
        expense_cents,
        net_worth_cents: balances.net_worth_cents,
    };
    Ok((snapshot, baseline))
}

/// Turn the Inbox's action items (+ a subscription-change summary) into review
/// flags. Pure mapping — no new checks invented here.
fn build_flags(action_items: &[ActionItem], subscription_change_count: i64) -> Vec<CloseFlag> {
    let mut flags: Vec<CloseFlag> = action_items
        .iter()
        .map(|a| CloseFlag {
            id: a.id.clone(),
            category: a.category.clone(),
            priority: a.priority.clone(),
            title: a.title.clone(),
            detail: a.detail.clone(),
            action_route: a.action_route.clone(),
            count: a.badge_count,
            acknowledged: false,
        })
        .collect();
    if subscription_change_count > 0 {
        flags.push(CloseFlag {
            id: "subscription-changes".into(),
            category: "bills".into(),
            priority: "medium".into(),
            title: if subscription_change_count == 1 {
                "1 subscription changed price".into()
            } else {
                format!("{subscription_change_count} subscriptions changed price")
            },
            detail: "Review recurring charges whose price moved this month.".into(),
            action_route: "/recurring".into(),
            count: Some(subscription_change_count),
            acknowledged: false,
        });
    }
    flags
}

fn drift_line(label: &str, recorded: i64, current: i64) -> DriftLine {
    let changed = recorded != 0
        && ((current - recorded).abs() as f64 / recorded.unsigned_abs() as f64) * 100.0 >= DRIFT_THRESHOLD_PCT;
    DriftLine { label: label.into(), recorded_cents: recorded, current_cents: current, changed_materially: changed }
}

struct CloseRow {
    status: String,
    notes: Option<String>,
    completed_at: Option<String>,
    snapshot_json: String,
    baseline_json: Option<String>,
    close_json: Option<String>,
}

fn load_row(
    conn: &rusqlite::Connection,
    year: i32,
    month: i32,
) -> Result<Option<CloseRow>, finsight_core::CoreError> {
    let row = conn
        .query_row(
            "SELECT status, notes, completed_at, snapshot_json, baseline_json, close_json
             FROM monthly_reviews WHERE year = ?1 AND month = ?2",
            rusqlite::params![year, month],
            |r| {
                Ok(CloseRow {
                    status: r.get(0)?,
                    notes: r.get(1)?,
                    completed_at: r.get(2)?,
                    snapshot_json: r.get(3)?,
                    baseline_json: r.get(4)?,
                    close_json: r.get(5)?,
                })
            },
        )
        .optional()?;
    Ok(row)
}

/// The close for a given month: live figures/flags while unopened or in
/// progress; the frozen record (plus drift) once completed.
pub async fn get_month_close(state: &ApiState, year: i32, month: i32) -> AppResult<MonthCloseView> {
    let action_items = inbox::get_action_items(state).await?;
    let db = (*state.db).clone();
    run(&db, move |conn| {
        validate_month(month)?;
        let (live_snapshot, live_baseline) = compute_snapshot(conn, year, month)?;
        let live_flags = build_flags(&action_items, live_snapshot.subscription_change_count);
        let row = load_row(conn, year, month)?;

        let view = match row {
            None => MonthCloseView {
                year, month, month_label: month_label(year, month),
                status: "not_started".into(), notes: None, completed_at: None,
                snapshot: live_snapshot, flags: live_flags, drift: Vec::new(),
            },
            Some(r) if r.status == "completed" => {
                let frozen: MonthCloseSnapshot = serde_json::from_str(&r.snapshot_json).unwrap_or_default();
                let frozen_flags: Vec<CloseFlag> = r
                    .close_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok())
                    .unwrap_or_default();
                // Drift: recorded baseline (if any) vs recomputed-now.
                // Drift only compares the MONTH-SCOPED flows — income and
                // spending — because those move only when a transaction dated in
                // the closed month is edited (exactly the record-tampering signal
                // criterion 4 wants). Net worth is a live balance-sheet figure
                // that drifts every day regardless of any edit to this month, so
                // it stays a recorded checkpoint in the snapshot but is never a
                // drift line — else the view would cry wolf on every closed month.
                let drift = match r.baseline_json.as_deref().and_then(|s| serde_json::from_str::<CloseBaseline>(s).ok()) {
                    Some(base) => vec![
                        drift_line("Income", base.income_cents, live_baseline.income_cents),
                        drift_line("Spending", base.expense_cents, live_baseline.expense_cents),
                    ]
                    .into_iter()
                    .filter(|d| d.changed_materially)
                    .collect(),
                    None => Vec::new(), // legacy row: viewable, not drift-comparable
                };
                MonthCloseView {
                    year, month, month_label: month_label(year, month),
                    status: r.status, notes: r.notes, completed_at: r.completed_at,
                    snapshot: frozen, flags: frozen_flags, drift,
                }
            }
            Some(r) => MonthCloseView {
                // in_progress or skipped — review must reflect reality, so live.
                year, month, month_label: month_label(year, month),
                status: r.status, notes: r.notes, completed_at: r.completed_at,
                snapshot: live_snapshot, flags: live_flags, drift: Vec::new(),
            },
        };
        Ok(view)
    })
    .await
    .map_err(AppError::from)
}

/// Advance the close lifecycle. `completed` freezes the snapshot, baseline, and
/// acknowledged flags; `in_progress`/`skipped` only move status + notes and
/// leave any prior frozen record intact (reopen keeps history; re-completing
/// re-freezes explicitly).
pub async fn save_month_close(state: &ApiState, input: SaveMonthCloseInput) -> AppResult<MonthCloseView> {
    let SaveMonthCloseInput { year, month, status, notes, acknowledged_flag_ids } = input;
    if !matches!(status.as_str(), "in_progress" | "completed" | "skipped") {
        return Err(AppError::from(finsight_core::CoreError::InvalidState(
            "status must be in_progress, completed, or skipped".into(),
        )));
    }
    // Action items are only frozen on completion — skip the work otherwise.
    let action_items = if status == "completed" {
        inbox::get_action_items(state).await?
    } else {
        Vec::new()
    };

    let db = (*state.db).clone();
    let (y, m, st, nt, ack) = (year, month, status.clone(), notes.clone(), acknowledged_flag_ids);
    run(&db, move |conn| {
        validate_month(m)?;
        let (snapshot, baseline) = compute_snapshot(conn, y, m)?;
        let snapshot_json = serde_json::to_string(&snapshot).unwrap_or_default();
        let now = chrono::Utc::now().to_rfc3339();

        match st.as_str() {
            "completed" => {
                let mut flags = build_flags(&action_items, snapshot.subscription_change_count);
                for f in &mut flags {
                    f.acknowledged = ack.contains(&f.id);
                }
                let baseline_json = serde_json::to_string(&baseline).unwrap_or_default();
                let close_json = serde_json::to_string(&flags).unwrap_or_default();
                conn.execute(
                    "INSERT INTO monthly_reviews(id, year, month, notes, snapshot_json, created_at, status, completed_at, baseline_json, close_json)
                     VALUES(?1,?2,?3,?4,?5,?6,'completed',?6,?7,?8)
                     ON CONFLICT(year, month) DO UPDATE SET
                       notes=excluded.notes, snapshot_json=excluded.snapshot_json,
                       status='completed', completed_at=excluded.completed_at,
                       baseline_json=excluded.baseline_json, close_json=excluded.close_json",
                    rusqlite::params![Uuid::new_v4().to_string(), y, m, &nt, snapshot_json, now, baseline_json, close_json],
                )?;
            }
            other => {
                // in_progress / skipped: move status + notes only, preserving any
                // prior frozen snapshot/baseline/close so reopening keeps history.
                conn.execute(
                    "INSERT INTO monthly_reviews(id, year, month, notes, snapshot_json, created_at, status)
                     VALUES(?1,?2,?3,?4,?5,?6,?7)
                     ON CONFLICT(year, month) DO UPDATE SET notes=excluded.notes, status=excluded.status",
                    rusqlite::params![Uuid::new_v4().to_string(), y, m, &nt, snapshot_json, now, other],
                )?;
            }
        }
        Ok::<(), finsight_core::CoreError>(())
    })
    .await
    .map_err(AppError::from)?;

    get_month_close(state, year, month).await
}

/// The calendar month immediately before `(year, month)`, wrapping the year.
fn previous_month(year: i32, month: i32) -> (i32, i32) {
    if month <= 1 { (year - 1, 12) } else { (year, month - 1) }
}

/// Standing-condition producer (#57): remind the user to close the month that
/// just ended, until a completed close exists for it. Stable dedup key so it is
/// raised once and resolved when the close completes; `expires_at` at the next
/// month rollover so a skipped reminder lapses rather than lingering. In-app /
/// badge only (the Today card is the universal surface) — no push.
///
/// Idempotent; safe to run every sweep cycle. Covers CSV-only users too since it
/// reads the reviews table, not a bank connection.
pub fn refresh_month_end_reminder(
    conn: &mut rusqlite::Connection,
    now: chrono::DateTime<chrono::Utc>,
) -> finsight_core::error::CoreResult<()> {
    use chrono::Datelike;
    let today = now.date_naive();
    // The month that just ended is the one worth nudging about.
    let (y, m) = previous_month(today.year(), today.month() as i32);
    let key = format!("month_end.{y}-{m:02}");

    let completed = conn
        .query_row(
            "SELECT 1 FROM monthly_reviews WHERE year=?1 AND month=?2 AND status='completed'",
            rusqlite::params![y, m],
            |_| Ok(()),
        )
        .optional()?
        .is_some();

    if completed {
        finsight_core::notify::resolve(conn, &key)?;
        return Ok(());
    }

    let prefs = finsight_core::notify::load_prefs(conn);
    // Lapse at the start of next month relative to today.
    let (ny, nm) = if today.month() >= 12 { (today.year() + 1, 1) } else { (today.year(), today.month() as i32 + 1) };
    let expires_at = Some(format!("{ny}-{nm:02}-01T00:00:00+00:00"));
    finsight_core::notify::enqueue(
        conn,
        finsight_core::notify::NewNotification {
            category: finsight_core::notify::NotificationCategory::MonthEndReview,
            urgency: finsight_core::notify::Urgency::Low,
            dedup_key: key,
            title: "Close out last month".into(),
            body: format!("{} is ready to review — verify the month's data and record its snapshot.", month_label(y, m)),
            sensitive: None,
            route: Some("/close".into()),
            expires_at,
        },
        &prefs,
        now,
    )?;
    Ok(())
}

/// Past closes, newest first — the "revisit a recorded close" surface the old
/// list_monthly_reviews never had a screen for.
pub async fn list_month_closes(state: &ApiState) -> AppResult<Vec<MonthCloseListItem>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let mut stmt = conn.prepare(
            "SELECT year, month, status, completed_at, snapshot_json
             FROM monthly_reviews ORDER BY year DESC, month DESC",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i32>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            let (year, month, status, completed_at, snapshot_json) = row;
            let snap: MonthCloseSnapshot = serde_json::from_str(&snapshot_json).unwrap_or_default();
            out.push(MonthCloseListItem {
                year, month, month_label: month_label(year, month), status, completed_at,
                savings_rate_pct: snap.savings_rate_pct,
                net_worth_cents: snap.net_worth_cents,
            });
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("mc.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_account(conn: &rusqlite::Connection) {
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Checking','Chk','USD','#fff',datetime('now'))", []).unwrap();
    }

    fn txn(conn: &rusqlite::Connection, date: &str, cents: i64, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
             VALUES(hex(randomblob(16)),'a',?1,?2,?3,0,'cleared',datetime('now'))",
            rusqlite::params![format!("{date}T12:00:00Z"), cents, merchant],
        )
        .unwrap();
    }

    fn unresolved_month_end(conn: &rusqlite::Connection) -> i64 {
        // Queries the table directly to sidestep list()'s wall-clock expiry filter.
        conn.query_row(
            "SELECT COUNT(*) FROM notifications WHERE category='month_end_review' AND resolved_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn previous_month_wraps_the_year() {
        assert_eq!(previous_month(2026, 1), (2025, 12));
        assert_eq!(previous_month(2026, 7), (2026, 6));
    }

    #[test]
    fn drift_line_flags_only_material_moves() {
        assert!(!drift_line("x", 100_000, 105_000).changed_materially, "5% is not material");
        assert!(drift_line("x", 100_000, 120_000).changed_materially, "20% is material");
        assert!(!drift_line("x", 0, 5_000).changed_materially, "no baseline → never material");
    }

    /// Brand-new user, one empty month: the close-bundle must degrade to zeros
    /// and empty lists, never crash or panic on missing budgets/goals/debt.
    #[test]
    fn compute_snapshot_degrades_on_a_sparse_month() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_account(&conn);
        let (snap, base) = compute_snapshot(&mut conn, 2026, 3).unwrap();
        assert_eq!(snap.income_cents, 0);
        assert_eq!(snap.expense_cents, 0);
        assert_eq!(snap.savings_cents, 0);
        assert!(snap.over_budget_categories.is_empty());
        assert!(snap.goal_progress.is_empty());
        assert_eq!(snap.subscription_change_count, 0);
        assert_eq!(base.income_cents, 0);
    }

    #[test]
    fn compute_snapshot_reflects_the_months_cashflow() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_account(&conn);
        txn(&conn, "2026-03-05", 500_000, "Payroll");
        txn(&conn, "2026-03-10", -120_000, "Rent");
        // A different month must not leak in.
        txn(&conn, "2026-04-02", -999_999, "Not this month");
        let (snap, _) = compute_snapshot(&mut conn, 2026, 3).unwrap();
        assert_eq!(snap.income_cents, 500_000);
        assert_eq!(snap.expense_cents, 120_000);
        assert_eq!(snap.savings_cents, 380_000);
        assert!(snap.savings_rate_pct > 0);
    }

    /// The optional #57 reminder is a standing condition: raised once for the
    /// month that just ended, deduped while unresolved, and resolved the moment
    /// a completed close exists for it.
    #[test]
    fn month_end_reminder_raises_until_closed_then_resolves() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        // now = 2026-04-10 → the month that just ended is March 2026.
        let now = chrono::Utc.with_ymd_and_hms(2026, 4, 10, 12, 0, 0).unwrap();

        refresh_month_end_reminder(&mut conn, now).unwrap();
        assert_eq!(unresolved_month_end(&conn), 1, "raised for the unclosed month");

        // Idempotent — a second sweep does not duplicate.
        refresh_month_end_reminder(&mut conn, now).unwrap();
        assert_eq!(unresolved_month_end(&conn), 1);

        // Record a completed close for March → the reminder resolves.
        conn.execute(
            "INSERT INTO monthly_reviews(id,year,month,snapshot_json,created_at,status,completed_at) \
             VALUES('r',2026,3,'{}',datetime('now'),'completed',datetime('now'))",
            [],
        )
        .unwrap();
        refresh_month_end_reminder(&mut conn, now).unwrap();
        assert_eq!(unresolved_month_end(&conn), 0, "resolved once March is closed");
    }
}
