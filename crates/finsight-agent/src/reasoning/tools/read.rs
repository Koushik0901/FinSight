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
            "Get detected recurring bills with expected next date"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"days_ahead": {"type": "integer", "default": 30}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let _days = args["days_ahead"].as_i64().unwrap_or(30);
            let cutoff = (chrono::Utc::now() - chrono::Duration::days(395))
                .format("%Y-%m-%d")
                .to_string();
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
            "Find transactions by merchant, date range, category, or amount"
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"merchant": {"type": "string"}, "start_date": {"type": "string"}, "end_date": {"type": "string"}, "limit": {"type": "integer", "default": 10}}})
        }
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
