use crate::finance;
use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn get_account_balances() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_account_balances"
        }
        fn description(&self) -> &str {
            "Get current balance for every account plus total"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT a.name, COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance \
                 FROM accounts a WHERE a.archived_at IS NULL ORDER BY a.name"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                Ok(json!({"name": r.get::<_, String>(0)?, "balance_cents": r.get::<_, i64>(1)?}))
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows
                .iter()
                .filter_map(|r| r["balance_cents"].as_i64())
                .sum();
            Ok(json!({"accounts": rows, "total_cents": total}))
        }
    }
    Arc::new(T)
}

pub fn get_net_worth() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_net_worth"
        }
        fn description(&self) -> &str {
            "Get current net worth: assets (confirmed account balances plus manual assets) minus liabilities. Accounts without a confirmed balance are reported separately as unknown and excluded from the total — mention them as unknown, never as $0."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let b = finsight_core::repos::net_worth::breakdown(ctx.conn)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            Ok(serde_json::to_value(b)?)
        }
    }
    Arc::new(T)
}

pub fn get_month_totals() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_month_totals"
        }
        fn description(&self) -> &str {
            "Get this month's income, expenses, and savings rate"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let (income, expense): (i64, i64) = ctx.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0), \
                        COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0) \
                 FROM transactions WHERE posted_at >= ?1",
                rusqlite::params![month_start],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let savings_rate = if income > 0 {
                ((income - expense) * 100 / income).max(0)
            } else {
                0
            };
            Ok(
                json!({"income_cents": income, "expense_cents": expense, "net_cents": income - expense, "savings_rate_pct": savings_rate}),
            )
        }
    }
    Arc::new(T)
}

pub fn get_top_spending_categories() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_top_spending_categories"
        }
        fn description(&self) -> &str {
            "Get top spending categories with amounts"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"limit": {"type": "integer", "default": 5}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let limit = args["limit"].as_i64().unwrap_or(5);
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let mut stmt = ctx.conn.prepare(
                "SELECT c.label, SUM(ABS(t.amount_cents)) AS spent \
                 FROM transactions t JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 \
                 GROUP BY c.id ORDER BY spent DESC LIMIT ?2",
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![month_start, limit], |r| {
                Ok(json!({"category": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"categories": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_spending_breakdown() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_spending_breakdown"
        }
        fn description(&self) -> &str {
            "Where the money goes over a window of months: top spending categories, top merchants, and per-month spend totals. Use for 'where am I spending the most' and overspending questions."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"months": {"type": "integer", "default": 6}, "limit": {"type": "integer", "default": 8}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let months = args["months"].as_i64().unwrap_or(6).clamp(1, 60);
            let limit = args["limit"].as_i64().unwrap_or(8).clamp(1, 25);
            // Window start = first day of the month, `months - 1` months back.
            let now = chrono::Utc::now().date_naive();
            let start = {
                use chrono::Datelike;
                let total = now.year() * 12 + (now.month0() as i32) - (months as i32 - 1);
                let y = total.div_euclid(12);
                let m = total.rem_euclid(12) as u32 + 1;
                chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(now)
            };
            let start_str = start.format("%Y-%m-%d").to_string();

            let mut cat_stmt = ctx.conn.prepare(
                "SELECT COALESCE(c.label, 'Uncategorized') AS label, SUM(ABS(t.amount_cents)) AS spent \
                 FROM transactions t LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND t.posted_at >= ?1 \
                 GROUP BY label ORDER BY spent DESC LIMIT ?2",
            )?;
            let top_categories: Vec<Value> = cat_stmt
                .query_map(rusqlite::params![start_str, limit], |r| {
                    Ok(json!({"category": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut merch_stmt = ctx.conn.prepare(
                "SELECT t.merchant_raw, SUM(ABS(t.amount_cents)) AS spent \
                 FROM transactions t \
                 WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND t.posted_at >= ?1 \
                 GROUP BY t.merchant_raw ORDER BY spent DESC LIMIT ?2",
            )?;
            let top_merchants: Vec<Value> = merch_stmt
                .query_map(rusqlite::params![start_str, limit], |r| {
                    Ok(json!({"merchant": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut month_stmt = ctx.conn.prepare(
                "SELECT substr(t.posted_at, 1, 7) AS ym, SUM(ABS(t.amount_cents)) AS spent \
                 FROM transactions t \
                 WHERE t.amount_cents < 0 AND t.is_transfer = 0 AND t.posted_at >= ?1 \
                 GROUP BY ym ORDER BY ym ASC",
            )?;
            let monthly: Vec<Value> = month_stmt
                .query_map(rusqlite::params![start_str], |r| {
                    Ok(json!({"month": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            // Complete window total = sum of per-month spend (top_categories is
            // capped at `limit`, so its sum would undercount).
            let total_spent_cents: i64 = monthly
                .iter()
                .filter_map(|m| m["spent_cents"].as_i64())
                .sum();

            Ok(json!({
                "window_months": months,
                "window_start": start_str,
                "top_categories": top_categories,
                "top_merchants": top_merchants,
                "monthly": monthly,
                "total_spent_cents": total_spent_cents,
            }))
        }
    }
    Arc::new(T)
}

pub fn get_budgets() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_budgets"
        }
        fn description(&self) -> &str {
            "Get current month budgets with budgeted vs actual"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let now = chrono::Utc::now();
            let month = now.format("%Y-%m").to_string();
            let month_start = now.format("%Y-%m-01").to_string();
            let mut stmt = ctx.conn.prepare(
                "SELECT c.label, b.amount_cents, \
                        COALESCE((SELECT SUM(ABS(t.amount_cents)) FROM transactions t \
                                  WHERE t.category_id = b.category_id AND t.amount_cents < 0 AND t.posted_at >= ?1), 0) AS spent \
                 FROM budgets b JOIN categories c ON c.id = b.category_id WHERE b.month = ?2"
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![month_start, month], |r| {
                Ok(json!({"category": r.get::<_, String>(0)?, "budget_cents": r.get::<_, i64>(1)?, "spent_cents": r.get::<_, i64>(2)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"budgets": rows, "month": month}))
        }
    }
    Arc::new(T)
}

pub fn get_goals() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_goals"
        }
        fn description(&self) -> &str {
            "Get goals with current balance, target, monthly contribution"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT name, target_cents, current_cents, monthly_cents FROM goals WHERE archived_at IS NULL ORDER BY sort_order"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                let target: i64 = r.get(1)?;
                let current: i64 = r.get(2)?;
                let pct = if target > 0 { current * 100 / target } else { 0 };
                Ok(json!({"name": r.get::<_, String>(0)?, "target_cents": target, "current_cents": current, "monthly_cents": r.get::<_, i64>(3)?, "progress_pct": pct}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"goals": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_recurring_bills() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_recurring_bills"
        }
        fn description(&self) -> &str {
            "Get detected recurring commitments — subscriptions and bills — classified deterministically with amount stability, cadence regularity, and vendor evidence. Each item has a kind, confidence, and reasons. Repeat purchases (groceries/dining/ride-hailing) and internal transfers/card payments are excluded. Use for 'what subscriptions am I paying for' and upcoming-bill questions."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"days_ahead": {"type": "integer", "default": 30}}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            use finsight_core::recurring::{detect_recurring, RecurringKind};
            let items = detect_recurring(ctx.conn, 395)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let rows: Vec<Value> = items
                .into_iter()
                .filter(|i| {
                    matches!(
                        i.kind,
                        RecurringKind::Subscription | RecurringKind::Bill | RecurringKind::Income
                    )
                })
                .map(|i| {
                    let kind = match i.kind {
                        RecurringKind::Subscription => "subscription",
                        RecurringKind::Bill => "bill",
                        RecurringKind::Income => "income",
                        _ => "other",
                    };
                    json!({
                        "merchant": i.display_merchant,
                        "kind": kind,
                        "confidence": (i.confidence * 100.0).round() / 100.0,
                        "reasons": i.reasons,
                        "median_amount_cents": i.median_amount_cents,
                        "last_amount_cents": i.last_amount_cents,
                        "avg_gap_days": (i.avg_gap_days * 10.0).round() / 10.0,
                        "cadence": i.cadence,
                        "occurrences": i.occurrences,
                        "last_seen": i.last_seen,
                        "next_expected": i.next_expected,
                        "category": i.category_label,
                    })
                })
                .collect();
            let subscription_count = rows.iter().filter(|r| r["kind"] == "subscription").count();
            let bill_count = rows.iter().filter(|r| r["kind"] == "bill").count();
            Ok(json!({
                "recurring": rows,
                "subscription_count": subscription_count,
                "bill_count": bill_count
            }))
        }
    }
    Arc::new(T)
}

pub fn get_liabilities() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_liabilities"
        }
        fn description(&self) -> &str {
            "Get credit cards and loans with balance, APR, minimum payment"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT id, name, liability_type, balance_cents, apr_pct, limit_cents, min_payment_cents, payoff_date FROM liabilities ORDER BY balance_cents DESC"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                Ok(json!({"id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "liability_type": r.get::<_, String>(2)?, "balance_cents": r.get::<_, i64>(3)?, "apr_pct": r.get::<_, Option<f64>>(4)?, "limit_cents": r.get::<_, Option<i64>>(5)?, "min_payment_cents": r.get::<_, Option<i64>>(6)?, "payoff_date": r.get::<_, Option<String>>(7)?}))
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows
                .iter()
                .filter_map(|r| r["balance_cents"].as_i64())
                .sum();
            Ok(json!({"liabilities": rows, "total_cents": total}))
        }
    }
    Arc::new(T)
}

pub fn search_transactions() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "search_transactions"
        }
        fn description(&self) -> &str {
            "Find and total transactions by merchant, date range, account, category, amount threshold, or direction. Returns each row's date, merchant, amount, account, and category, plus the count and summed total. Use min_amount_cents for 'over $N' questions (it filters on the absolute amount). Use direction='expense' or 'income' to restrict sign."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {
                "merchant": {"type": "string"},
                "account": {"type": "string", "description": "account name substring"},
                "start_date": {"type": "string", "description": "inclusive YYYY-MM-DD"},
                "end_date": {"type": "string", "description": "inclusive YYYY-MM-DD"},
                "min_amount_cents": {"type": "integer", "description": "minimum absolute amount in cents (e.g. 6000 for over $60)"},
                "direction": {"type": "string", "enum": ["any", "expense", "income"], "default": "any"},
                "limit": {"type": "integer", "default": 50}
            }})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let mut sql = "SELECT t.merchant_raw, t.amount_cents, t.posted_at, COALESCE(c.label, 'Uncategorized'), COALESCE(a.name, 'Unknown account') \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 LEFT JOIN accounts a ON a.id = t.account_id \
                 WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            if let Some(m) = args["merchant"].as_str() {
                sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
                params.push(Box::new(format!("%{}%", m)));
            }
            if let Some(acct) = args["account"].as_str() {
                sql.push_str(" AND lower(a.name) LIKE lower(?)");
                params.push(Box::new(format!("%{}%", acct)));
            }
            if let Some(s) = args["start_date"].as_str() {
                sql.push_str(" AND t.posted_at >= ?");
                params.push(Box::new(s.to_string()));
            }
            if let Some(e) = args["end_date"].as_str() {
                sql.push_str(" AND t.posted_at <= ?");
                params.push(Box::new(format!("{}T23:59:59", e)));
            }
            if let Some(min) = args["min_amount_cents"].as_i64() {
                sql.push_str(" AND ABS(t.amount_cents) >= ?");
                params.push(Box::new(min.abs()));
            }
            match args["direction"].as_str() {
                Some("expense") => sql.push_str(" AND t.amount_cents < 0"),
                Some("income") => sql.push_str(" AND t.amount_cents > 0"),
                _ => {}
            }
            // Cap to keep payloads bounded even for large ranges.
            let limit = args["limit"].as_i64().unwrap_or(50).clamp(1, 500);
            sql.push_str(" ORDER BY t.posted_at DESC LIMIT ?");
            params.push(Box::new(limit));

            let mut stmt = ctx.conn.prepare(&sql)?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())), |r| {
                Ok(json!({
                    "date": r.get::<_, String>(2)?,
                    "merchant": r.get::<_, String>(0)?,
                    "amount_cents": r.get::<_, i64>(1)?,
                    "account": r.get::<_, String>(4)?,
                    "category": r.get::<_, String>(3)?
                }))
            })?.filter_map(|r| r.ok()).collect();
            let total_cents: i64 = rows.iter().filter_map(|r| r["amount_cents"].as_i64()).sum();
            let total_abs_cents: i64 = rows
                .iter()
                .filter_map(|r| r["amount_cents"].as_i64())
                .map(|v| v.abs())
                .sum();
            Ok(json!({
                "transactions": rows,
                "count": rows.len(),
                "total_cents": total_cents,
                "total_abs_cents": total_abs_cents,
                "capped": rows.len() as i64 == limit
            }))
        }
    }
    Arc::new(T)
}

pub fn find_anomalies() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "find_anomalies"
        }
        fn description(&self) -> &str {
            "List transactions flagged as unusual/anomalous (statistically out of pattern), with the reason. Use for 'any unusual charges', 'weird transactions', or fraud-check style questions."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"limit": {"type": "integer", "default": 20}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let limit = args["limit"].as_i64().unwrap_or(20).clamp(1, 100);
            let mut stmt = ctx.conn.prepare(
                "SELECT substr(t.posted_at,1,10), t.merchant_raw, t.amount_cents, \
                        COALESCE(c.label,'Uncategorized'), COALESCE(a.name,'Unknown account'), \
                        COALESCE(t.ai_explanation,'') \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 LEFT JOIN accounts a ON a.id = t.account_id \
                 WHERE t.is_anomaly = 1 \
                 ORDER BY t.posted_at DESC LIMIT ?1",
            )?;
            let rows: Vec<Value> = stmt
                .query_map(rusqlite::params![limit], |r| {
                    Ok(json!({
                        "date": r.get::<_, String>(0)?,
                        "merchant": r.get::<_, String>(1)?,
                        "amount_cents": r.get::<_, i64>(2)?,
                        "category": r.get::<_, String>(3)?,
                        "account": r.get::<_, String>(4)?,
                        "reason": r.get::<_, String>(5)?
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();
            Ok(json!({"anomalies": rows, "count": rows.len()}))
        }
    }
    Arc::new(T)
}

pub fn list_uncategorized_transactions() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "list_uncategorized_transactions"
        }
        fn description(&self) -> &str {
            "List transactions that still have no category, plus the available categories to choose from. Use this before draft_recategorization: pick a category id for each transaction from available_categories. Returns a bounded page; total_uncategorized is the full count."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"limit": {"type": "integer", "default": 50}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let limit = args["limit"].as_i64().unwrap_or(50).clamp(1, 100);
            let total_uncategorized: i64 = ctx.conn.query_row(
                "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND is_transfer = 0",
                [],
                |r| r.get(0),
            )?;
            let mut txn_stmt = ctx.conn.prepare(
                "SELECT t.id, t.merchant_raw, t.amount_cents, substr(t.posted_at,1,10), COALESCE(a.name,'Unknown account') \
                 FROM transactions t LEFT JOIN accounts a ON a.id = t.account_id \
                 WHERE t.category_id IS NULL AND t.is_transfer = 0 \
                 ORDER BY t.posted_at DESC LIMIT ?1",
            )?;
            let uncategorized: Vec<Value> = txn_stmt
                .query_map(rusqlite::params![limit], |r| {
                    Ok(json!({
                        "id": r.get::<_, String>(0)?,
                        "merchant": r.get::<_, String>(1)?,
                        "amount_cents": r.get::<_, i64>(2)?,
                        "date": r.get::<_, String>(3)?,
                        "account": r.get::<_, String>(4)?
                    }))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut cat_stmt = ctx.conn.prepare(
                "SELECT id, label FROM categories WHERE archived_at IS NULL ORDER BY label",
            )?;
            let available_categories: Vec<Value> = cat_stmt
                .query_map([], |r| {
                    Ok(json!({"id": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(json!({
                "total_uncategorized": total_uncategorized,
                "returned": uncategorized.len(),
                "uncategorized": uncategorized,
                "available_categories": available_categories
            }))
        }
    }
    Arc::new(T)
}

pub fn run_cashflow_projection() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_cashflow_projection"
        }
        fn description(&self) -> &str {
            "Project runway and end-of-month net under hypothetical changes"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"months": {"type": "integer", "default": 3}, "extra_monthly_expense_cents": {"type": "integer", "default": 0}, "extra_monthly_income_cents": {"type": "integer", "default": 0}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let months = args["months"].as_i64().unwrap_or(3);
            let extra_expense = args["extra_monthly_expense_cents"].as_i64().unwrap_or(0);
            let extra_income = args["extra_monthly_income_cents"].as_i64().unwrap_or(0);
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let day_of_month = now.format("%d").to_string().parse::<i64>().unwrap_or(15);
            let (income, expense): (i64, i64) = ctx.conn.query_row(
                "SELECT COALESCE(SUM(CASE WHEN amount_cents>0 THEN amount_cents ELSE 0 END),0), \
                        COALESCE(SUM(CASE WHEN amount_cents<0 THEN -amount_cents ELSE 0 END),0) \
                 FROM transactions WHERE posted_at >= ?1",
                rusqlite::params![month_start],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?;
            let balance: i64 = ctx.conn.query_row(
                "SELECT COALESCE(SUM(balance_cents), 0) FROM accounts WHERE archived_at IS NULL",
                [],
                |r| r.get(0),
            )?;
            let daily_net = if day_of_month > 0 {
                (income - expense) / day_of_month
            } else {
                0
            };
            let avg_daily_burn = if day_of_month > 0 {
                expense / day_of_month
            } else {
                0
            };
            let runway_days = if avg_daily_burn > 0 {
                balance / avg_daily_burn
            } else {
                9999
            };
            let projected_monthly_net = (income + extra_income) - (expense + extra_expense);
            let projections: Vec<Value> = (1..=months).map(|m| {
                json!({"month": m, "projected_net_cents": projected_monthly_net * m, "projected_balance_cents": balance + projected_monthly_net * m})
            }).collect();
            Ok(
                json!({"current_balance_cents": balance, "monthly_income_cents": income, "monthly_expense_cents": expense, "daily_net_cents": daily_net, "runway_days": runway_days, "projections": projections}),
            )
        }
    }
    Arc::new(T)
}

pub fn get_financial_snapshot() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_financial_snapshot"
        }
        fn description(&self) -> &str {
            "Get the full local finance snapshot for planning: liquid balances, cashflow, goals, debts, recurring bills, planned transactions, and data warnings"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            Ok(serde_json::to_value(finance::build_snapshot(ctx.conn)?)?)
        }
    }
    Arc::new(T)
}

pub fn analyze_cash_inflow() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "analyze_cash_inflow"
        }
        fn description(&self) -> &str {
            "Deterministically allocate a paycheck or windfall across emergency fund, high-interest debt, goals, and investing eligibility"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"amount_cents":{"type":"integer"}},"required":["amount_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let amount = args["amount_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("amount_cents required"))?;
            Ok(serde_json::to_value(finance::analyze_cash_inflow(
                ctx.conn, amount,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn calculate_goal_eta() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "calculate_goal_eta"
        }
        fn description(&self) -> &str {
            "Calculate exact ETA for a goal given a weekly, biweekly, semimonthly, or monthly contribution"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"goal_id":{"type":"string"},"contribution_cents":{"type":"integer"},"cadence":{"type":"string","enum":["weekly","biweekly","semimonthly","monthly"]}},"required":["goal_id","contribution_cents","cadence"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let goal_id = args["goal_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("goal_id required"))?;
            let contribution = args["contribution_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("contribution_cents required"))?;
            let cadence = args["cadence"].as_str().unwrap_or("monthly");
            Ok(serde_json::to_value(finance::calculate_goal_eta(
                ctx.conn,
                goal_id,
                contribution,
                cadence,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn rank_debt_payoff() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "rank_debt_payoff"
        }
        fn description(&self) -> &str {
            "Rank debts for payoff using avalanche by APR or snowball by balance"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"method":{"type":"string","enum":["avalanche","snowball"]}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let method = args["method"].as_str().unwrap_or("avalanche");
            Ok(serde_json::to_value(finance::rank_debt_payoff(
                ctx.conn, method,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn compare_debt_vs_goal() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "compare_debt_vs_goal"
        }
        fn description(&self) -> &str {
            "Compare preserving a savings goal against using some goal savings or paycheck surplus for debt"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"goal_id":{"type":"string"},"liability_id":{"type":"string"}},"required":["goal_id"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let goal_id = args["goal_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("goal_id required"))?;
            let liability_id = args["liability_id"].as_str();
            Ok(serde_json::to_value(finance::compare_debt_vs_goal(
                ctx.conn,
                goal_id,
                liability_id,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn run_debt_payoff_scenarios() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_debt_payoff_scenarios"
        }
        fn description(&self) -> &str {
            "Run deterministic payoff scenarios for all active debts using avalanche or snowball, minimum payments, APRs, and optional extra monthly debt payment"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"method":{"type":"string","enum":["avalanche","snowball"],"default":"avalanche"},"extra_monthly_payment_cents":{"type":"integer","default":0}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let method = args["method"].as_str().unwrap_or("avalanche");
            let extra = args["extra_monthly_payment_cents"].as_i64().unwrap_or(0);
            Ok(serde_json::to_value(finance::run_debt_payoff_scenarios(
                ctx.conn, method, extra,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn run_goal_allocation_scenarios() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_goal_allocation_scenarios"
        }
        fn description(&self) -> &str {
            "Allocate available monthly savings across goals by priority, deadline, or proportional strategy and estimate goal ETAs"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"monthly_available_cents":{"type":"integer"},"strategy":{"type":"string","enum":["priority","deadline","proportional"],"default":"priority"}},"required":["monthly_available_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let available = args["monthly_available_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("monthly_available_cents required"))?;
            let strategy = args["strategy"].as_str().unwrap_or("priority");
            Ok(serde_json::to_value(
                finance::run_goal_allocation_scenarios(ctx.conn, available, strategy)?,
            )?)
        }
    }
    Arc::new(T)
}

pub fn run_goal_conflict_scenario() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_goal_conflict_scenario"
        }
        fn description(&self) -> &str {
            "Compare a proposed goal contribution against upcoming bills, planned transactions, monthly surplus, and the emergency floor"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"goal_id":{"type":"string"},"contribution_cents":{"type":"integer"}},"required":["goal_id","contribution_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let goal_id = args["goal_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("goal_id required"))?;
            let contribution = args["contribution_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("contribution_cents required"))?;
            Ok(serde_json::to_value(finance::run_goal_conflict_scenario(
                ctx.conn,
                goal_id,
                contribution,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn run_emergency_fund_scenarios() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_emergency_fund_scenarios"
        }
        fn description(&self) -> &str {
            "Model one-, three-, and six-month emergency fund targets, gaps, runway, and time to target for a monthly contribution"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"monthly_contribution_cents":{"type":"integer","default":0}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let contribution = args["monthly_contribution_cents"].as_i64().unwrap_or(0);
            Ok(serde_json::to_value(
                finance::run_emergency_fund_scenarios(ctx.conn, contribution)?,
            )?)
        }
    }
    Arc::new(T)
}

pub fn run_cashflow_timeline() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_cashflow_timeline"
        }
        fn description(&self) -> &str {
            "Build a deterministic month-by-month cashflow timeline from local average income, expenses, planned transactions, and liquid balance"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"months":{"type":"integer","default":3}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let months = args["months"].as_i64().unwrap_or(3);
            Ok(serde_json::to_value(finance::run_cashflow_timeline(
                ctx.conn, months,
            )?)?)
        }
    }
    Arc::new(T)
}

pub fn run_purchase_affordability() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "run_purchase_affordability"
        }
        fn description(&self) -> &str {
            "Model whether a one-time purchase is affordable using emergency cash, monthly surplus, high-interest debt, and wait/save alternatives"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"purchase_amount_cents":{"type":"integer"}},"required":["purchase_amount_cents"]})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let amount = args["purchase_amount_cents"]
                .as_i64()
                .ok_or_else(|| anyhow::anyhow!("purchase_amount_cents required"))?;
            Ok(serde_json::to_value(finance::run_purchase_affordability(
                ctx.conn, amount,
            )?)?)
        }
    }
    Arc::new(T)
}
pub fn get_data_quality_report() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_data_quality_report"
        }
        fn description(&self) -> &str {
            "Report missing finance data that can weaken planning answers, including APRs, minimum payments, uncategorized expenses, goals, debts, and planned transactions"
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            Ok(serde_json::to_value(finance::get_data_quality_report(
                ctx.conn,
            )?)?)
        }
    }
    Arc::new(T)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::messages::{AgentChange, AgentDraftAction};
    use finsight_core::{db::run_migrations, keychain, Db};
    use rusqlite::Connection;
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("tools.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn call(conn: &mut Connection, tool: Arc<dyn Tool>, args: Value) -> Value {
        let mut changes: Vec<AgentChange> = Vec::new();
        let mut drafts: Vec<AgentDraftAction> = Vec::new();
        let mut ctx = ToolContext {
            conn,
            changes: &mut changes,
            draft_actions: &mut drafts,
        };
        tool.execute(&mut ctx, args).unwrap()
    }

    fn seed_txns(conn: &mut Connection) {
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('chk','Me','Bank','Checking','Everyday Checking','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('amex','Me','Amex','Credit','Amex Card','USD','#111',datetime('now'))", []).unwrap();
        // (account, date, amount_cents, merchant)
        let rows = [
            ("chk", "2026-01-15", -9_999, "Costco"),      // Jan, over $60 expense
            ("chk", "2026-02-10", -4_200, "Tim Hortons"), // under $60
            ("amex", "2026-03-05", -12_050, "Best Buy"),  // over $60
            ("amex", "2026-06-28", -6_100, "Uber"),       // June, just over $60
            ("chk", "2026-07-02", -20_000, "Rent"),       // OUT of range (July)
            ("chk", "2026-03-01", 300_000, "Payroll"),    // income, over $60 but positive
        ];
        for (acct, date, amt, merch) in rows {
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
                 VALUES(hex(randomblob(16)), ?1, ?2, ?3, ?4, 'cleared', datetime('now'))",
                rusqlite::params![acct, date, amt, merch],
            )
            .unwrap();
        }
    }

    #[test]
    fn search_transactions_filters_by_date_range_and_amount_threshold() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_txns(&mut conn);

        // Jan–June 2026, expenses over $60.
        let out = call(
            &mut conn,
            search_transactions(),
            json!({
                "start_date": "2026-01-01",
                "end_date": "2026-06-30",
                "min_amount_cents": 6000,
                "direction": "expense",
                "limit": 500
            }),
        );
        let txns = out["transactions"].as_array().unwrap();
        // Costco (Jan), Best Buy (Mar), Uber (Jun) — NOT Tim Hortons ($42),
        // NOT Rent (July, out of range), NOT Payroll (income).
        assert_eq!(out["count"].as_i64().unwrap(), 3, "got: {txns:?}");
        let merchants: Vec<&str> = txns.iter().map(|t| t["merchant"].as_str().unwrap()).collect();
        assert!(merchants.contains(&"Costco"));
        assert!(merchants.contains(&"Best Buy"));
        assert!(merchants.contains(&"Uber"));
        assert!(!merchants.contains(&"Tim Hortons"));
        assert!(!merchants.contains(&"Rent"));
        assert!(!merchants.contains(&"Payroll"));
        // Rows carry account + category; total is grounded.
        assert!(txns.iter().all(|t| t["account"].is_string() && t["category"].is_string()));
        assert_eq!(out["total_abs_cents"].as_i64().unwrap(), 9_999 + 12_050 + 6_100);
    }

    #[test]
    fn search_transactions_filters_by_account() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_txns(&mut conn);
        let out = call(
            &mut conn,
            search_transactions(),
            json!({"account": "amex", "limit": 500}),
        );
        let txns = out["transactions"].as_array().unwrap();
        assert!(txns.iter().all(|t| t["account"].as_str().unwrap() == "Amex Card"));
        assert_eq!(out["count"].as_i64().unwrap(), 2);
    }

    #[test]
    fn spending_breakdown_reports_categories_merchants_and_months() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_txns(&mut conn);
        // Wide window so all seeded 2026 spend is included.
        let out = call(&mut conn, get_spending_breakdown(), json!({"months": 60}));
        assert!(out["top_merchants"].as_array().unwrap().len() >= 3);
        assert!(!out["monthly"].as_array().unwrap().is_empty());
        // total_spent_cents excludes the positive Payroll row.
        assert!(out["total_spent_cents"].as_i64().unwrap() > 0);
    }

    #[test]
    fn find_anomalies_returns_flagged_transactions_with_reasons() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('a','Me','Bank','Checking','Chk','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_anomaly,ai_explanation,status,created_at) VALUES('n1','a','2026-05-01T00:00:00Z',-99900,'Unknown LLC',1,'10x larger than typical spend here','cleared',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_anomaly,status,created_at) VALUES('n2','a','2026-05-02T00:00:00Z',-1200,'Coffee',0,'cleared',datetime('now'))", []).unwrap();

        let out = call(&mut conn, find_anomalies(), json!({}));
        assert_eq!(out["count"].as_i64().unwrap(), 1);
        let a = &out["anomalies"].as_array().unwrap()[0];
        assert_eq!(a["merchant"].as_str().unwrap(), "Unknown LLC");
        assert!(a["reason"].as_str().unwrap().contains("larger"));
    }

    #[test]
    fn net_worth_tool_marks_unknown_balances() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        // A manual account with a confirmed balance.
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) VALUES('a1','Me','Bank','Checking','Checking','USD','#fff','manual',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id, as_of_date, balance_cents, source) VALUES('a1','2026-06-01',500000,'manual')", []).unwrap();
        let out = call(&mut conn, get_net_worth(), json!({}));
        assert_eq!(out["known_account_balance_cents"].as_i64().unwrap(), 500000);
        assert_eq!(out["net_worth_cents"].as_i64().unwrap(), 500000);
        assert!(out["has_data"].as_bool().unwrap());
    }
}
