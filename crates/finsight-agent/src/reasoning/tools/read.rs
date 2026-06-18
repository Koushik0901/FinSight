use crate::reasoning::tools::{Tool, ToolContext};
use anyhow::Result;
use serde_json::{json, Value};
use std::sync::Arc;

pub fn get_account_balances() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_account_balances" }
        fn description(&self) -> &str { "Get current balance for every account plus total" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT a.name, COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance \
                 FROM accounts a WHERE a.archived_at IS NULL ORDER BY a.name"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                Ok(json!({"name": r.get::<_, String>(0)?, "balance_cents": r.get::<_, i64>(1)?}))
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows.iter().filter_map(|r| r["balance_cents"].as_i64()).sum();
            Ok(json!({"accounts": rows, "total_cents": total}))
        }
    }
    Arc::new(T)
}

pub fn get_month_totals() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_month_totals" }
        fn description(&self) -> &str { "Get this month's income, expenses, and savings rate" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
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
            let savings_rate = if income > 0 { ((income - expense) * 100 / income).max(0) } else { 0 };
            Ok(json!({"income_cents": income, "expense_cents": expense, "net_cents": income - expense, "savings_rate_pct": savings_rate}))
        }
    }
    Arc::new(T)
}

pub fn get_top_spending_categories() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_top_spending_categories" }
        fn description(&self) -> &str { "Get top spending categories with amounts" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"limit": {"type": "integer", "default": 5}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let limit = args["limit"].as_i64().unwrap_or(5);
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            let mut stmt = ctx.conn.prepare(
                "SELECT c.label, SUM(ABS(t.amount_cents)) AS spent \
                 FROM transactions t JOIN categories c ON c.id = t.category_id \
                 WHERE t.amount_cents < 0 AND t.posted_at >= ?1 \
                 GROUP BY c.id ORDER BY spent DESC LIMIT ?2"
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![month_start, limit], |r| {
                Ok(json!({"category": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"categories": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_budgets() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_budgets" }
        fn description(&self) -> &str { "Get current month budgets with budgeted vs actual" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
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
        fn name(&self) -> &str { "get_goals" }
        fn description(&self) -> &str { "Get goals with current balance, target, monthly contribution" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
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
        fn name(&self) -> &str { "get_recurring_bills" }
        fn description(&self) -> &str { "Get detected recurring bills with expected next date" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"days_ahead": {"type": "integer", "default": 30}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let _days = args["days_ahead"].as_i64().unwrap_or(30);
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(395)).format("%Y-%m-%d").to_string();
            let mut stmt = ctx.conn.prepare(
                "WITH gaps AS ( \
                    SELECT merchant_raw, date(posted_at) AS d, LAG(date(posted_at)) OVER (PARTITION BY merchant_raw ORDER BY posted_at) AS prev_d \
                    FROM transactions WHERE posted_at >= ?1 \
                 ), agg AS ( \
                    SELECT merchant_raw, AVG(julianday(d)-julianday(prev_d)) AS avg_gap, MAX(d) AS last_seen, MAX(amount_cents) AS last_amount, COUNT(*) AS occ \
                    FROM gaps WHERE prev_d IS NOT NULL GROUP BY merchant_raw HAVING occ >= 2 AND AVG(julianday(d)-julianday(prev_d)) BETWEEN 5 AND 400 \
                 ) SELECT merchant_raw, avg_gap, last_seen, last_amount FROM agg ORDER BY ABS(last_amount) DESC"
            )?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params![cutoff], |r| {
                let avg_gap: f64 = r.get(1)?;
                let last_seen: String = r.get(2)?;
                let next = chrono::NaiveDate::parse_from_str(&last_seen, "%Y-%m-%d")
                    .map(|d| (d + chrono::Duration::days(avg_gap.round() as i64)).format("%Y-%m-%d").to_string())
                    .unwrap_or(last_seen.clone());
                Ok(json!({"merchant": r.get::<_, String>(0)?, "avg_gap_days": avg_gap, "last_seen": last_seen, "next_expected": next, "last_amount_cents": r.get::<_, i64>(3)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"recurring_bills": rows}))
        }
    }
    Arc::new(T)
}

pub fn get_liabilities() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "get_liabilities" }
        fn description(&self) -> &str { "Get credit cards and loans with balance, APR, minimum payment" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {}}) }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let mut stmt = ctx.conn.prepare(
                "SELECT name, balance_cents, apr_pct, limit_cents FROM liabilities WHERE archived_at IS NULL ORDER BY balance_cents DESC"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                Ok(json!({"name": r.get::<_, String>(0)?, "balance_cents": r.get::<_, i64>(1)?, "apr_pct": r.get::<_, f64>(2)?, "limit_cents": r.get::<_, Option<i64>>(3)?}))
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows.iter().filter_map(|r| r["balance_cents"].as_i64()).sum();
            Ok(json!({"liabilities": rows, "total_cents": total}))
        }
    }
    Arc::new(T)
}

pub fn search_transactions() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "search_transactions" }
        fn description(&self) -> &str { "Find transactions by merchant, date range, category, or amount" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"merchant": {"type": "string"}, "start_date": {"type": "string"}, "end_date": {"type": "string"}, "limit": {"type": "integer", "default": 10}}}) }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let mut sql = "SELECT t.merchant_raw, t.amount_cents, t.posted_at, COALESCE(c.label, 'Uncategorized') FROM transactions t LEFT JOIN categories c ON c.id = t.category_id WHERE 1=1".to_string();
            let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
            if let Some(m) = args["merchant"].as_str() {
                sql.push_str(" AND lower(t.merchant_raw) LIKE lower(?)");
                params.push(Box::new(format!("%{}%", m)));
            }
            if let Some(s) = args["start_date"].as_str() {
                sql.push_str(" AND t.posted_at >= ?");
                params.push(Box::new(s.to_string()));
            }
            if let Some(e) = args["end_date"].as_str() {
                sql.push_str(" AND t.posted_at <= ?");
                params.push(Box::new(format!("{}T23:59:59", e)));
            }
            let limit = args["limit"].as_i64().unwrap_or(10);
            sql.push_str(" ORDER BY t.posted_at DESC LIMIT ?");
            params.push(Box::new(limit));

            let mut stmt = ctx.conn.prepare(&sql)?;
            let rows: Vec<Value> = stmt.query_map(rusqlite::params_from_iter(params.iter().map(|b| b.as_ref())), |r| {
                Ok(json!({"merchant": r.get::<_, String>(0)?, "amount_cents": r.get::<_, i64>(1)?, "date": r.get::<_, String>(2)?, "category": r.get::<_, String>(3)?}))
            })?.filter_map(|r| r.ok()).collect();
            Ok(json!({"transactions": rows, "count": rows.len()}))
        }
    }
    Arc::new(T)
}

pub fn run_cashflow_projection() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str { "run_cashflow_projection" }
        fn description(&self) -> &str { "Project runway and end-of-month net under hypothetical changes" }
        fn parameters(&self) -> Value { json!({"type": "object", "properties": {"months": {"type": "integer", "default": 3}, "extra_monthly_expense_cents": {"type": "integer", "default": 0}, "extra_monthly_income_cents": {"type": "integer", "default": 0}}}) }
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
                "SELECT COALESCE(SUM(balance_cents), 0) FROM accounts WHERE archived_at IS NULL", [], |r| r.get(0)
            )?;
            let daily_net = if day_of_month > 0 { (income - expense) / day_of_month } else { 0 };
            let avg_daily_burn = if day_of_month > 0 { expense / day_of_month } else { 0 };
            let runway_days = if avg_daily_burn > 0 { balance / avg_daily_burn } else { 9999 };
            let projected_monthly_net = (income + extra_income) - (expense + extra_expense);
            let projections: Vec<Value> = (1..=months).map(|m| {
                json!({"month": m, "projected_net_cents": projected_monthly_net * m, "projected_balance_cents": balance + projected_monthly_net * m})
            }).collect();
            Ok(json!({"current_balance_cents": balance, "monthly_income_cents": income, "monthly_expense_cents": expense, "daily_net_cents": daily_net, "runway_days": runway_days, "projections": projections}))
        }
    }
    Arc::new(T)
}
