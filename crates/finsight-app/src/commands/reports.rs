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

fn month_short_label(ym: &str) -> String {
    // ym = "YYYY-MM"
    let month_num: u32 = ym[5..7].parse().unwrap_or(1);
    let names = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    names[(month_num.saturating_sub(1)) as usize].to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn get_report_data(
    state: tauri::State<'_, AppState>,
    scope: String,
) -> AppResult<ReportData> {
    let db = (*state.db).clone();

    run(&db, move |conn| {
        let now = chrono::Utc::now();

        // Build the list of YYYY-MM strings for this scope
        let months: Vec<String> = match scope.as_str() {
            "month" => {
                vec![now.format("%Y-%m").to_string()]
            }
            "quarter" => (0..3i32)
                .map(|i| {
                    let m0 = now.month0() as i32 - i;
                    let (yr, mo) = if m0 < 0 {
                        (now.year() - 1, (m0 + 12) as u32 + 1)
                    } else {
                        (now.year(), m0 as u32 + 1)
                    };
                    format!("{yr}-{mo:02}")
                })
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect(),
            "all" => {
                let oldest: Option<String> = conn
                    .query_row(
                        "SELECT strftime('%Y-%m', MIN(posted_at)) FROM transactions",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap_or(None);
                if let Some(oldest_str) = oldest {
                    let oldest_y: i32 = oldest_str[..4].parse().unwrap_or(now.year());
                    let oldest_m: i32 = oldest_str[5..7].parse().unwrap_or(1);
                    let cur_y = now.year();
                    let cur_m = now.month() as i32;
                    let total_months = (cur_y - oldest_y) * 12 + (cur_m - oldest_m) + 1;
                    let n = total_months.clamp(1, 24) as usize;
                    (0..n)
                        .map(|i| {
                            let months_back = i as i32;
                            let m0 = cur_m - 1 - months_back;
                            let (yr, mo) = if m0 < 0 {
                                let back = (-m0 - 1) / 12 + 1;
                                (cur_y - back, ((m0 + back * 12) as u32) + 1)
                            } else {
                                (cur_y, m0 as u32 + 1)
                            };
                            format!("{yr}-{mo:02}")
                        })
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect()
                } else {
                    vec![now.format("%Y-%m").to_string()]
                }
            }
            _ => {
                // "year" default: Jan through current month of this year
                let cur_m = now.month() as usize;
                (1..=cur_m)
                    .map(|m| format!("{}-{:02}", now.year(), m))
                    .collect()
            }
        };

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
                let mut stmt = conn.prepare(
                    "SELECT strftime('%Y-%m', posted_at) AS mo,
                        SUM(CASE WHEN amount_cents > 0 THEN amount_cents  ELSE 0 END),
                        SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END)
                 FROM transactions
                 WHERE strftime('%Y-%m', posted_at) >= ?1
                   AND strftime('%Y-%m', posted_at) <= ?2
                   AND is_transfer = 0
                 GROUP BY mo
                 ORDER BY mo",
                )?;
                // Every row must be readable — a corrupt page or query failure
                // partway through must surface as a real error, not silently
                // drop rows and render a fabricated $0 for the affected months.
                let db_rows: std::collections::HashMap<String, (i64, i64)> = stmt
                    .query_map(rusqlite::params![first, last], |r| {
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

        let top_categories = {
            let mut stmt = conn.prepare(
                "SELECT c.id, c.label, COALESCE(c.color,''), \
                        SUM(-t.amount_cents) AS total, COUNT(t.id) \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND t.posted_at >= ?1 AND t.posted_at < ?2 \
                 GROUP BY c.id, c.label, c.color \
                 ORDER BY total DESC \
                 LIMIT 10",
            )?;
            let rows = stmt.query_map(rusqlite::params![scope_start, scope_end], |r| {
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
            rows.collect::<Result<Vec<_>, rusqlite::Error>>()?
        };

        let top_merchants = {
            let mut stmt = conn.prepare(
                "SELECT t.merchant_raw, COALESCE(c.label,''), COALESCE(c.color,''), \
                        SUM(-t.amount_cents) AS total, COUNT(t.id) \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND t.posted_at >= ?1 AND t.posted_at < ?2 \
                 GROUP BY t.merchant_raw \
                 ORDER BY total DESC \
                 LIMIT 10",
            )?;
            let rows = stmt.query_map(rusqlite::params![scope_start, scope_end], |r| {
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
        let (income, expense, txn_count): (i64, i64, i64) = conn.query_row(
            "SELECT \
               COALESCE(SUM(CASE WHEN amount_cents > 0 AND is_transfer = 0 THEN amount_cents  ELSE 0 END), 0), \
               COALESCE(SUM(CASE WHEN amount_cents < 0 AND is_transfer = 0 THEN -amount_cents ELSE 0 END), 0), \
               COUNT(*) \
             FROM transactions WHERE posted_at >= ?1",
            rusqlite::params![this_month_start],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )?;
        let net = income - expense;
        let savings_rate = if income > 0 { (net * 100) / income } else { 0 };
        Ok(MonthTotals {
            income_cents: income,
            expense_cents: expense,
            net_cents: net,
            savings_rate_pct: savings_rate,
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
        let mut stmt = conn.prepare(
            "SELECT
                strftime('%Y-%m', posted_at) AS month,
                COALESCE(SUM(CASE WHEN amount_cents > 0 THEN amount_cents ELSE 0 END), 0) AS income,
                COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0) AS expense
             FROM transactions
             WHERE posted_at >= ?1 AND is_transfer = 0
             GROUP BY month
             ORDER BY month ASC",
        )?;
        let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
        })?;
        let mut out = Vec::new();
        for row in rows.flatten() {
            let (month, income, expense) = row;
            let net = income - expense;
            let savings_rate_pct = if income > 0 {
                (net.max(0) * 100) / income
            } else {
                0
            };
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
        let (income_cents, expense_cents): (i64, i64) = conn.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN amount_cents > 0 THEN amount_cents ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0)
             FROM transactions
             WHERE posted_at >= ?1 AND posted_at < ?2 AND is_transfer = 0",
            rusqlite::params![&month_start, &month_end],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let net = income_cents - expense_cents;
        let savings_rate_pct = if income_cents > 0 {
            (net.max(0) * 100) / income_cents
        } else {
            0
        };

        let month_str = format!("{}-{:02}", input.year, input.month);
        let over_budget_categories: Vec<String> = {
            let mut stmt = conn.prepare(
                "WITH actuals AS (
                   SELECT category_id, SUM(ABS(amount_cents)) AS spent
                   FROM transactions
                   WHERE posted_at >= ?1 AND posted_at < ?2 AND amount_cents < 0 AND is_transfer = 0
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
