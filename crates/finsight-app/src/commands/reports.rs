use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::{Datelike, Utc};
use finsight_core::repos::run;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// One month's summary for the bar chart.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthSummary {
    /// "YYYY-MM"
    pub month: String,
    /// Human label e.g. "Jan"
    pub label: String,
    /// Total inflows (positive transactions), as positive cents
    pub income_cents: i64,
    /// Total outflows (negative transactions), as positive cents
    pub expense_cents: i64,
    /// Net = income - expense
    pub net_cents: i64,
}

/// One category's 12-month total.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryTotal {
    pub category_id: String,
    pub label: String,
    pub color: String,
    pub total_cents: i64,
    pub txn_count: i64,
}

/// One merchant's 12-month total.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MerchantTotal {
    pub merchant_raw: String,
    pub category_label: String,
    pub category_color: String,
    pub total_cents: i64,
    pub txn_count: i64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReportData {
    pub monthly: Vec<MonthSummary>,
    pub monthly_last_year: Vec<MonthSummary>,
    pub top_categories: Vec<CategoryTotal>,
    pub top_merchants: Vec<MerchantTotal>,
}

/// Normalize a (year, zero-based-month-index) pair where the index may be
/// negative or ≥12, returning (year, 1-based-month) with year carry.
fn normalize_month(year: i32, mut m0: i32) -> (i32, u32) {
    let mut y = year;
    while m0 < 0 {
        m0 += 12;
        y -= 1;
    }
    while m0 >= 12 {
        m0 -= 12;
        y += 1;
    }
    (y, (m0 as u32) + 1)
}

/// Build the list of `YYYY-MM` months for a report scope, anchored on the data's
/// most recent activity (`anchor_y`/`anchor_m`) rather than wall-clock now, so
/// historical imports still produce populated charts. `oldest_ym` bounds the
/// "all" scope. Returned oldest→newest.
pub(crate) fn scope_month_list(
    scope: &str,
    anchor_y: i32,
    anchor_m: i32,
    oldest_ym: Option<&str>,
) -> Vec<String> {
    let ending = |count: i32| -> Vec<String> {
        (0..count.max(1))
            .map(|i| {
                let (yr, mo) = normalize_month(anchor_y, (anchor_m - 1) - i);
                format!("{yr}-{mo:02}")
            })
            .rev()
            .collect()
    };
    match scope {
        "month" => vec![format!("{anchor_y}-{anchor_m:02}")],
        "quarter" => ending(3),
        "all" => {
            let count = oldest_ym
                .and_then(|s| {
                    let oy: i32 = s.get(0..4)?.parse().ok()?;
                    let om: i32 = s.get(5..7)?.parse().ok()?;
                    Some((anchor_y - oy) * 12 + (anchor_m - om) + 1)
                })
                .unwrap_or(1);
            ending(count.clamp(1, 24))
        }
        _ => ending(12), // "year"
    }
}

fn month_short_label(ym: &str) -> String {
    // ym = "YYYY-MM"
    let month_num: u32 = ym[5..7].parse().unwrap_or(1);
    let names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    names[(month_num.saturating_sub(1)) as usize].to_string()
}

/// Per-category expense totals over `[start, end)`, optionally weighted to one
/// household member's ownership share. A `settle_up = 1` reimbursement inflow
/// nets against the category's total (matching metrics.rs cashflow) instead of
/// being silently dropped by an `amount_cents < 0`-only filter. Extracted from
/// [`get_report_data`] so it's directly unit-testable without a Tauri
/// `AppState`.
fn category_totals_for_window(
    conn: &rusqlite::Connection,
    member: Option<&str>,
    start: &str,
    end: &str,
) -> finsight_core::CoreResult<Vec<CategoryTotal>> {
    let owner_join: &str = if member.is_some() {
        " JOIN (SELECT ao.account_id, 1.0 / oc.n AS weight FROM account_owners ao \
          JOIN (SELECT account_id, COUNT(*) AS n FROM account_owners GROUP BY account_id) oc \
          ON oc.account_id = ao.account_id WHERE ao.member_id = ?) w ON w.account_id = t.account_id"
    } else {
        ""
    };
    let wmul: &str = if member.is_some() { "* w.weight" } else { "" };

    let mut cat_binds: Vec<String> = Vec::new();
    if let Some(m) = member {
        cat_binds.push(m.to_string());
    }
    cat_binds.push(start.to_string());
    cat_binds.push(end.to_string());

    let sql = format!(
        "SELECT c.id, c.label, COALESCE(c.color,''), \
                CAST(ROUND(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents {wmul} \
                                    WHEN t.amount_cents < 0 THEN -t.amount_cents {wmul} \
                                    ELSE 0 END)) AS INTEGER) AS total, COUNT(t.id) \
         FROM transactions t{owner_join} \
         LEFT JOIN categories c ON c.id = t.category_id \
         WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ? AND t.posted_at < ? \
         GROUP BY c.id, c.label, c.color \
         ORDER BY total DESC \
         LIMIT 10"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(cat_binds.iter()), |r| {
        Ok(CategoryTotal {
            // Uncategorized spending groups on a NULL category id via the
            // LEFT JOIN; represent it with an empty id rather than
            // failing the whole report on the NULL.
            category_id: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
            label: r
                .get::<_, Option<String>>(1)?
                .unwrap_or_else(|| "Uncategorized".to_string()),
            color: r.get(2)?,
            total_cents: r.get(3)?,
            txn_count: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, rusqlite::Error>>()?)
}

#[tauri::command]
#[specta::specta]
pub async fn get_report_data(
    state: tauri::State<'_, AppState>,
    scope: String,
    member_id: Option<String>,
) -> AppResult<ReportData> {
    let db = (*state.db).clone();

    run(&db, move |conn| {
        let now = chrono::Utc::now();

        // Per-member scoping: when a member is selected, JOIN the ownership-weight
        // subquery (1/owner_count, joint accounts split equally) and multiply
        // amounts by the share. `None` leaves the SQL unweighted (household). The
        // member id, when present, is always the FIRST bound `?` (it appears in
        // the JOIN, ahead of the date bounds).
        let member = member_id.clone();
        let owner_join: &str = if member.is_some() {
            " JOIN (SELECT ao.account_id, 1.0 / oc.n AS weight FROM account_owners ao \
              JOIN (SELECT account_id, COUNT(*) AS n FROM account_owners GROUP BY account_id) oc \
              ON oc.account_id = ao.account_id WHERE ao.member_id = ?) w ON w.account_id = t.account_id"
        } else {
            ""
        };
        let wmul: &str = if member.is_some() { "* w.weight" } else { "" };

        // Anchor the report windows on the most recent MONTH WITH ACTIVITY, not
        // wall-clock now. Imported statements are often historical, so anchoring
        // on "now" makes the default month/quarter/year charts empty even though
        // the data is there. Anchoring on the data makes every scope populate.
        let anchor_ym: Option<String> = conn
            .query_row(
                "SELECT strftime('%Y-%m', MAX(posted_at)) FROM transactions",
                [],
                |r| r.get(0),
            )
            .unwrap_or(None);
        let (anchor_y, anchor_m): (i32, i32) = anchor_ym
            .as_deref()
            .and_then(|s| {
                let y = s.get(0..4)?.parse().ok()?;
                let m = s.get(5..7)?.parse().ok()?;
                Some((y, m))
            })
            .unwrap_or((now.year(), now.month() as i32));

        let oldest: Option<String> = conn
            .query_row(
                "SELECT strftime('%Y-%m', MIN(posted_at)) FROM transactions",
                [],
                |r| r.get(0),
            )
            .unwrap_or(None);

        // Build the list of YYYY-MM strings for this scope, anchored on the data.
        let months: Vec<String> =
            scope_month_list(scope.as_str(), anchor_y, anchor_m, oldest.as_deref());

        // Build monthly_last_year: same months offset back by 12
        let months_ly: Vec<String> = months
            .iter()
            .map(|m| {
                let yr: i32 = m[..4].parse().unwrap_or(2000);
                let mo = &m[5..];
                format!("{}-{}", yr - 1, mo)
            })
            .collect();

        let fetch_monthly =
            |month_list: &[String]| -> finsight_core::CoreResult<Vec<MonthSummary>> {
                if month_list.is_empty() {
                    return Ok(vec![]);
                }
                let first = &month_list[0];
                let last = &month_list[month_list.len() - 1];
                let sql = format!(
                    "SELECT strftime('%Y-%m', t.posted_at) AS mo, \
                        CAST(ROUND(SUM(CASE WHEN t.amount_cents > 0 AND t.settle_up = 0 THEN t.amount_cents {wmul} ELSE 0 END)) AS INTEGER), \
                        CAST(ROUND(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents {wmul} \
                                            WHEN t.amount_cents < 0 THEN -t.amount_cents {wmul} \
                                            ELSE 0 END)) AS INTEGER) \
                     FROM transactions t{owner_join} \
                     WHERE strftime('%Y-%m', t.posted_at) >= ? \
                       AND strftime('%Y-%m', t.posted_at) <= ? \
                       AND t.is_transfer = 0 \
                       AND {pred} \
                     GROUP BY mo \
                     ORDER BY mo",
                    pred = finsight_core::metrics::non_investment_txn_predicate("t")
                );
                let mut stmt = conn.prepare(&sql)?;
                // member (if any) is the leading bind, then the date bounds.
                let mut binds: Vec<String> = Vec::new();
                if let Some(m) = member.as_ref() {
                    binds.push(m.clone());
                }
                binds.push(first.clone());
                binds.push(last.clone());
                // Every row must be readable — a corrupt page or query failure
                // partway through must surface as a real error, not silently
                // drop rows and render a fabricated $0 for the affected months.
                let db_rows: std::collections::HashMap<String, (i64, i64)> = stmt
                    .query_map(rusqlite::params_from_iter(binds.iter()), |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, rusqlite::Error>>()?
                    .into_iter()
                    .map(|(mo, inc, exp)| (mo, (inc, exp)))
                    .collect();
                Ok(month_list
                    .iter()
                    .map(|m| {
                        let (inc, exp) = db_rows.get(m).copied().unwrap_or((0, 0));
                        MonthSummary {
                            label: month_short_label(m),
                            month: m.clone(),
                            income_cents: inc,
                            expense_cents: exp,
                            net_cents: inc - exp,
                        }
                    })
                    .collect())
            };

        let monthly = fetch_monthly(&months)?;
        let monthly_last_year = fetch_monthly(&months_ly)?;

        // scope date range for top_categories / top_merchants
        let scope_start = months
            .first()
            .map(|m| format!("{}-01", m))
            .unwrap_or_default();
        let scope_end = months
            .last()
            .map(|m| {
                let yr: i32 = m[..4].parse().unwrap_or(2000);
                let mo: u32 = m[5..7].parse().unwrap_or(1);
                let (ny, nm) = if mo == 12 {
                    (yr + 1, 1u32)
                } else {
                    (yr, mo + 1)
                };
                format!("{ny}-{nm:02}-01")
            })
            .unwrap_or_default();

        let top_categories =
            category_totals_for_window(conn, member.as_deref(), &scope_start, &scope_end)?;

        let top_merchants = {
            let mut binds: Vec<String> = Vec::new();
            if let Some(m) = member.as_ref() {
                binds.push(m.clone());
            }
            binds.push(scope_start.clone());
            binds.push(scope_end.clone());
            let sql = format!(
                "SELECT t.merchant_raw, COALESCE(c.label,''), COALESCE(c.color,''), \
                        CAST(ROUND(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents {wmul} \
                                            WHEN t.amount_cents < 0 THEN -t.amount_cents {wmul} \
                                            ELSE 0 END)) AS INTEGER) AS total, COUNT(t.id) \
                 FROM transactions t{owner_join} \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ? AND t.posted_at < ? \
                 GROUP BY t.merchant_raw \
                 ORDER BY total DESC \
                 LIMIT 10"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(rusqlite::params_from_iter(binds.iter()), |r| {
                Ok(MerchantTotal {
                    merchant_raw: r.get(0)?,
                    category_label: r.get(1)?,
                    category_color: r.get(2)?,
                    total_cents: r.get(3)?,
                    txn_count: r.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, rusqlite::Error>>()?
        };

        Ok(ReportData {
            monthly,
            monthly_last_year,
            top_categories,
            top_merchants,
        })
    })
    .await
    .map_err(AppError::from)
}

/// Lightweight summary for the Today screen.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthTotals {
    pub income_cents: i64,
    pub expense_cents: i64,
    pub net_cents: i64,
    pub savings_rate_pct: i64,
    /// Number of transactions this month
    pub txn_count: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn get_month_totals(state: tauri::State<'_, AppState>) -> AppResult<MonthTotals> {
    let db = (*state.db).clone();
    let this_month_start = Utc::now().format("%Y-%m-01").to_string();

    run(&db, move |conn| {
        // Income/expense/net/savings-rate come from the shared metrics layer so
        // "this month" reads identically here, on Today, and in the Copilot.
        let cashflow = finsight_core::metrics::cashflow_since(conn, &this_month_start)?;
        let txn_count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE posted_at >= ?1",
            rusqlite::params![this_month_start],
            |r| r.get(0),
        )?;
        Ok(MonthTotals {
            income_cents: cashflow.income_cents,
            expense_cents: cashflow.expense_cents,
            net_cents: cashflow.net_cents,
            savings_rate_pct: cashflow.savings_rate_pct,
            txn_count,
        })
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavingsRatePoint {
    pub month: String,
    pub savings_rate_pct: i64,
    pub income_cents: i64,
    pub expense_cents: i64,
}

#[tauri::command]
#[specta::specta]
pub async fn get_savings_rate_history(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<SavingsRatePoint>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(365))
            .format("%Y-%m-01")
            .to_string();
        let mut stmt = conn.prepare(&format!(
            "SELECT
                strftime('%Y-%m', posted_at) AS month,
                COALESCE(SUM(CASE WHEN amount_cents > 0 AND settle_up = 0 THEN amount_cents ELSE 0 END), 0) AS income,
                COALESCE(SUM(CASE WHEN settle_up = 1 THEN -amount_cents
                                  WHEN amount_cents < 0 THEN -amount_cents
                                  ELSE 0 END), 0) AS expense
             FROM transactions t
             WHERE posted_at >= ?1 AND is_transfer = 0 AND {}
             GROUP BY month
             ORDER BY month ASC",
            finsight_core::metrics::non_investment_txn_predicate("t")
        ))?;
        let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            let (month, income, expense) = row;
            // Honest signed rate from the shared formula: a deficit month dips
            // below zero on the sparkline instead of being flattened to 0%.
            let savings_rate_pct = finsight_core::metrics::savings_rate_pct(income, expense);
            out.push(SavingsRatePoint {
                month,
                savings_rate_pct,
                income_cents: income,
                expense_cents: expense,
            });
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyReviewSnapshot {
    pub income_cents: i64,
    pub expense_cents: i64,
    pub savings_rate_pct: i64,
    pub over_budget_categories: Vec<String>,
    pub goal_progress: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MonthlyReview {
    pub id: String,
    pub year: i32,
    pub month: i32,
    pub month_label: String,
    pub notes: Option<String>,
    pub snapshot: MonthlyReviewSnapshot,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateMonthlyReviewInput {
    pub year: i32,
    pub month: i32,
    pub notes: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn create_monthly_review(
    state: tauri::State<'_, AppState>,
    input: CreateMonthlyReviewInput,
) -> AppResult<MonthlyReview> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        if !(1..=12).contains(&input.month) {
            return Err(finsight_core::CoreError::InvalidState(
                "month must be between 1 and 12".to_string(),
            ));
        }
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let month_start = format!("{}-{:02}-01", input.year, input.month);
        let month_end = format!(
            "{}-{:02}-01",
            if input.month == 12 { input.year + 1 } else { input.year },
            if input.month == 12 { 1 } else { input.month + 1 }
        );
        // Through the one shared window metric (transfer + investment exclusions,
        // honest signed rate) instead of a private variant that clamped deficits
        // to 0% — a review of a deficit month must say so.
        let (income_cents, expense_cents) =
            finsight_core::metrics::income_expense_between(conn, &month_start, &month_end)?;
        let savings_rate_pct = finsight_core::metrics::savings_rate_pct(income_cents, expense_cents);

        let month_str = format!("{}-{:02}", input.year, input.month);
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
            let collected = stmt
                .query_map(rusqlite::params![&month_start, &month_end, &month_str], |r| {
                    r.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, rusqlite::Error>>()?;
            collected
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
                    } else {
                        0
                    }
                })
            })
            .collect();

        let snapshot = MonthlyReviewSnapshot {
            income_cents,
            expense_cents,
            savings_rate_pct,
            over_budget_categories,
            goal_progress,
        };
        let snapshot_json = serde_json::to_string(&snapshot).unwrap_or_default();

        conn.execute(
            "INSERT OR REPLACE INTO monthly_reviews(id, year, month, notes, snapshot_json, created_at)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                &id,
                input.year,
                input.month,
                &input.notes,
                snapshot_json,
                &now
            ],
        )?;

        let month_names = [
            "",
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        Ok(MonthlyReview {
            id,
            year: input.year,
            month: input.month,
            month_label: format!("{} {}", month_names[input.month as usize], input.year),
            notes: input.notes.clone(),
            snapshot,
            created_at: now,
        })
    })
    .await
    .map_err(AppError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_monthly_reviews(
    state: tauri::State<'_, AppState>,
) -> AppResult<Vec<MonthlyReview>> {
    let db = (*state.db).clone();
    run(&db, |conn| {
        let mut stmt = conn.prepare(
            "SELECT id, year, month, notes, snapshot_json, created_at
             FROM monthly_reviews
             ORDER BY year DESC, month DESC",
        )?;
        let month_names = [
            "",
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, i32>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            let (id, year, month, notes, snapshot_json, created_at) = row;
            let snapshot: MonthlyReviewSnapshot =
                serde_json::from_str(&snapshot_json).unwrap_or_default();
            out.push(MonthlyReview {
                id,
                year,
                month,
                month_label: format!(
                    "{} {}",
                    month_names.get(month as usize).copied().unwrap_or(""),
                    year
                ),
                notes,
                snapshot,
                created_at,
            });
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::{category_totals_for_window, normalize_month, scope_month_list};
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("reports.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_account(conn: &rusqlite::Connection) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();
    }

    fn seed_category(conn: &rusqlite::Connection, id: &str, label: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('grp', 'Group', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id, group_id, label, color, sort_order) VALUES(?1, 'grp', ?2, '#94A3B8', 0)",
            rusqlite::params![id, label],
        )
        .unwrap();
    }

    #[test]
    fn category_totals_net_settle_up_inflow() {
        // A settle_up = 1 reimbursement inflow must reduce the category's
        // reported total instead of being silently dropped by an
        // `amount_cents < 0`-only filter.
        let (_dir, db) = fresh_db();
        let conn = db.get().unwrap();
        seed_account(&conn);
        seed_category(&conn, "food", "Food");

        // Ordinary $50 grocery expense.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,is_transfer,created_at) \
             VALUES('e1','a1','2026-05-10T00:00:00Z',-5000,'GROCERY','food','cleared',0,0,'2026-05-10T00:00:00Z')",
            [],
        )
        .unwrap();
        // A $20 settle-up reimbursement for the same category.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,is_anomaly,is_transfer,created_at,settle_up) \
             VALUES('su1','a1','2026-05-12T00:00:00Z',2000,'FRIEND REFUND','food','cleared',0,0,'2026-05-12T00:00:00Z',1)",
            [],
        )
        .unwrap();

        let totals =
            category_totals_for_window(&conn, None, "2026-05-01", "2026-06-01").unwrap();
        let food = totals
            .iter()
            .find(|c| c.category_id == "food")
            .expect("food category present");
        assert_eq!(
            food.total_cents, 3000,
            "settle-up inflow nets against expense: 5000 - 2000 = 3000"
        );
    }

    #[test]
    fn normalize_month_carries_across_year_boundaries() {
        assert_eq!(normalize_month(2026, 0), (2026, 1)); // Jan
        assert_eq!(normalize_month(2026, -1), (2025, 12)); // back into prev year
        assert_eq!(normalize_month(2026, -13), (2024, 12));
        assert_eq!(normalize_month(2026, 12), (2027, 1)); // forward carry
    }

    #[test]
    fn month_scope_anchors_on_data_not_now() {
        // Data's most recent activity is 2026-07 even though "now" is irrelevant.
        assert_eq!(
            scope_month_list("month", 2026, 7, Some("2023-12")),
            vec!["2026-07"]
        );
    }

    #[test]
    fn year_scope_returns_12_months_ending_at_anchor() {
        let months = scope_month_list("year", 2026, 7, Some("2023-12"));
        assert_eq!(months.len(), 12);
        assert_eq!(months.first().unwrap(), "2025-08"); // 12 months back
        assert_eq!(months.last().unwrap(), "2026-07"); // anchor
    }

    #[test]
    fn quarter_scope_returns_3_months_ending_at_anchor_with_year_wrap() {
        // Anchor Feb 2026 → Dec 2025, Jan 2026, Feb 2026.
        let months = scope_month_list("quarter", 2026, 2, None);
        assert_eq!(months, vec!["2025-12", "2026-01", "2026-02"]);
    }

    #[test]
    fn all_scope_spans_oldest_to_anchor_clamped_to_24() {
        let months = scope_month_list("all", 2026, 7, Some("2023-12"));
        // 2023-12 .. 2026-07 is 32 months, clamped to 24.
        assert_eq!(months.len(), 24);
        assert_eq!(months.last().unwrap(), "2026-07");
    }
}
