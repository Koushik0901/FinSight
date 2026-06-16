use crate::error::{AppError, AppResult};
use crate::AppState;
use chrono::{Datelike, Utc};
use finsight_core::repos::run;
use serde::Serialize;
use specta::Type;

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
                    let n = total_months.min(24).max(1) as usize;
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
                 GROUP BY mo
                 ORDER BY mo",
                )?;
                let db_rows: std::collections::HashMap<String, (i64, i64)> = stmt
                    .query_map(rusqlite::params![first, last], |r| {
                        Ok((
                            r.get::<_, String>(0)?,
                            r.get::<_, i64>(1)?,
                            r.get::<_, i64>(2)?,
                        ))
                    })?
                    .filter_map(|r| r.ok())
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
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 AND t.posted_at < ?2 \
                 GROUP BY c.id, c.label, c.color \
                 ORDER BY total DESC \
                 LIMIT 10",
            )?;
            let rows = stmt.query_map(rusqlite::params![scope_start, scope_end], |r| {
                Ok(CategoryTotal {
                    category_id: r.get(0)?,
                    label: r
                        .get::<_, Option<String>>(1)?
                        .unwrap_or_else(|| "Uncategorized".to_string()),
                    color: r.get(2)?,
                    total_cents: r.get(3)?,
                    txn_count: r.get(4)?,
                })
            })?;
            let mut out = Vec::new();
            for row in rows {
                if let Ok(r) = row {
                    out.push(r);
                }
            }
            out
        };

        let top_merchants = {
            let mut stmt = conn.prepare(
                "SELECT t.merchant_raw, COALESCE(c.label,''), COALESCE(c.color,''), \
                        SUM(-t.amount_cents) AS total, COUNT(t.id) \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 AND t.posted_at < ?2 \
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
            let mut out = Vec::new();
            for row in rows {
                if let Ok(r) = row {
                    out.push(r);
                }
            }
            out
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
               COALESCE(SUM(CASE WHEN amount_cents > 0 THEN amount_cents  ELSE 0 END), 0), \
               COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0), \
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
