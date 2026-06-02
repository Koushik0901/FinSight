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
    /// Last 12 months, oldest first
    pub monthly: Vec<MonthSummary>,
    /// Top 10 categories by 12-month spend
    pub top_categories: Vec<CategoryTotal>,
    /// Top 10 merchants by 12-month spend
    pub top_merchants: Vec<MerchantTotal>,
}

fn month_short_label(ym: &str) -> String {
    // ym = "YYYY-MM"
    let month_num: u32 = ym[5..7].parse().unwrap_or(1);
    let names = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    names[(month_num.saturating_sub(1)) as usize].to_string()
}

#[tauri::command]
#[specta::specta]
pub async fn get_report_data(state: tauri::State<'_, AppState>) -> AppResult<ReportData> {
    let db = (*state.db).clone();

    run(&db, |conn| {
        // 12 calendar months going back from this month
        let now = Utc::now();
        let months: Vec<String> = (0..12)
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
            .collect();

        // Monthly totals
        let monthly = {
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
            let first = months.first().map(|s| s.as_str()).unwrap_or("1900-01");
            let last  = months.last().map(|s| s.as_str()).unwrap_or("2099-12");
            let db_rows: std::collections::HashMap<String, (i64, i64)> = stmt
                .query_map(rusqlite::params![first, last], |r| {
                    Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))
                })?
                .filter_map(|r| r.ok())
                .map(|(mo, inc, exp)| (mo, (inc, exp)))
                .collect();

            months.iter().map(|m| {
                let (inc, exp) = db_rows.get(m).copied().unwrap_or((0, 0));
                MonthSummary {
                    label: month_short_label(m),
                    month: m.clone(),
                    income_cents: inc,
                    expense_cents: exp,
                    net_cents: inc - exp,
                }
            }).collect()
        };

        // Top categories by 12-month outflow
        let top_categories = {
            let cutoff = months.first().map(|m| format!("{}-01", m)).unwrap_or_default();
            let mut stmt = conn.prepare(
                "SELECT c.id, c.label, COALESCE(c.color,''), \
                        SUM(-t.amount_cents) AS total, COUNT(t.id) \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 \
                 GROUP BY c.id, c.label, c.color \
                 ORDER BY total DESC \
                 LIMIT 10",
            )?;
            let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
                Ok(CategoryTotal {
                    category_id: r.get(0)?,
                    label: r.get::<_, Option<String>>(1)?.unwrap_or_else(|| "Uncategorized".to_string()),
                    color: r.get(2)?,
                    total_cents: r.get(3)?,
                    txn_count: r.get(4)?,
                })
            })?;
            let mut out: Vec<CategoryTotal> = Vec::new();
            for row in rows { if let Ok(r) = row { out.push(r); } }
            out
        };

        // Top merchants by 12-month outflow
        let top_merchants = {
            let cutoff = months.first().map(|m| format!("{}-01", m)).unwrap_or_default();
            let mut stmt = conn.prepare(
                "SELECT t.merchant_raw, COALESCE(c.label,''), COALESCE(c.color,''), \
                        SUM(-t.amount_cents) AS total, COUNT(t.id) \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 \
                 GROUP BY t.merchant_raw \
                 ORDER BY total DESC \
                 LIMIT 10",
            )?;
            let rows = stmt.query_map(rusqlite::params![cutoff], |r| {
                Ok(MerchantTotal {
                    merchant_raw: r.get(0)?,
                    category_label: r.get(1)?,
                    category_color: r.get(2)?,
                    total_cents: r.get(3)?,
                    txn_count: r.get(4)?,
                })
            })?;
            let mut out: Vec<MerchantTotal> = Vec::new();
            for row in rows { if let Ok(r) = row { out.push(r); } }
            out
        };

        Ok(ReportData { monthly, top_categories, top_merchants })
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
