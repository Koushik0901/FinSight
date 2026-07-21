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
            "Get the current balance for every account plus the total. Accounts \
             without a confirmed balance snapshot have balance_known=false and a \
             null balance — report those as unknown/unconfirmed, never as $0, and \
             note that the total excludes them."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            // No COALESCE-to-zero: an account with no balance snapshot must read
            // as UNKNOWN (null), not $0, so the model doesn't report a fabricated
            // zero for e.g. an unsynced brokerage.
            let mut stmt = ctx.conn.prepare(
                "SELECT a.name, (SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC, CASE source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END LIMIT 1) AS balance \
                 FROM accounts a WHERE a.archived_at IS NULL ORDER BY a.name"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                let name: String = r.get(0)?;
                let balance: Option<i64> = r.get(1)?;
                Ok(match balance {
                    Some(b) => json!({"name": name, "balance_cents": b, "balance_known": true}),
                    None => json!({"name": name, "balance_cents": null, "balance_known": false}),
                })
            })?.filter_map(|r| r.ok()).collect();
            let total: i64 = rows
                .iter()
                .filter_map(|r| r["balance_cents"].as_i64())
                .sum();
            let unknown_count = rows.iter().filter(|r| r["balance_known"] == json!(false)).count();
            Ok(json!({
                "accounts": rows,
                "total_cents": total,
                "note": if unknown_count > 0 {
                    format!("{unknown_count} account(s) have an unknown balance (no confirmed snapshot) and are excluded from total_cents; report them as unknown, not $0.")
                } else {
                    String::new()
                }
            }))
        }
    }
    Arc::new(T)
}

pub fn get_account_balance_history() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_account_balance_history"
        }
        fn description(&self) -> &str {
            "The highest and lowest balance an account has ever reached, and the \
             dates. Reconstructs the balance from transaction history rather than \
             reading recorded snapshots, so it finds the TRUE peak instead of \
             whichever recorded day happened to be highest. Use for 'the most I've \
             ever had in X', 'when was my savings highest', 'when did this account \
             bottom out'. Omit `account` to cover every account. \
             CRITICAL: when a result has amount_reliable=false, its DATES are still \
             correct but every dollar figure is off by an unknown constant — give \
             the timing and say the amount cannot be pinned down. Never present an \
             unreliable amount as if it were established. \
             Credit and Loan balances are stored NEGATIVE, so for those accounts \
             `trough` is when the most was OWED and `peak` is when the least was \
             owed — check account_type before describing which is which."
        }
        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "account": {
                        "type": "string",
                        "description": "Account name or part of one, e.g. 'savings'. Omit for all accounts."
                    },
                    "since": {
                        "type": "string",
                        "description": "Optional ISO date (YYYY-MM-DD). Restricts the window to on/after this date."
                    }
                }
            })
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let account = args["account"].as_str().filter(|s| !s.is_empty());
            let since = args["since"].as_str().filter(|s| !s.is_empty());

            let pattern = account.map(|a| format!("%{a}%"));
            let mut sql = String::from("SELECT id, type FROM accounts WHERE archived_at IS NULL");
            if pattern.is_some() {
                sql.push_str(" AND lower(name) LIKE lower(?1)");
            }
            sql.push_str(" ORDER BY name");

            // Propagate rather than `filter_map(ok)`: silently dropping an account
            // here would answer "your highest balance" from a subset of accounts
            // without ever saying one was missing.
            let ids: Vec<(String, String)> = {
                let mut stmt = ctx.conn.prepare(&sql)?;
                let row = |r: &rusqlite::Row| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?));
                let rows = match &pattern {
                    Some(p) => stmt.query_map(rusqlite::params![p], row)?.collect::<
                        std::result::Result<Vec<_>, _>,
                    >(),
                    None => stmt
                        .query_map([], row)?
                        .collect::<std::result::Result<Vec<_>, _>>(),
                };
                rows?
            };

            if ids.is_empty() {
                return Ok(json!({
                    "accounts": [],
                    "note": match account {
                        Some(a) => format!(
                            "No account matches '{a}'. Call again with no account to see them all, \
                             then ask which one they meant — do not guess."
                        ),
                        None => "There are no accounts yet.".to_string(),
                    }
                }));
            }

            let mut accounts = Vec::new();
            let mut unreliable = Vec::new();
            let mut skipped = Vec::new();
            for (id, account_type) in &ids {
                let tl = finsight_core::repos::accounts::balance_timeline(ctx.conn, id, since)?;
                if !tl.reconstructable {
                    let reason = tl.skip_reason.unwrap_or_else(|| "unavailable".to_string());
                    skipped.push(format!("{} ({reason})", tl.account_name));
                    continue;
                }
                let amount_reliable =
                    tl.anchor != finsight_core::models::BalanceAnchorQuality::AssumedZero;
                if !amount_reliable {
                    unreliable.push(tl.account_name.clone());
                }
                accounts.push(json!({
                    "account": tl.account_name,
                    "account_type": account_type,
                    "peak_cents": tl.peak.as_ref().map(|p| p.balance_cents),
                    "peak_date": tl.peak.as_ref().map(|p| p.date.as_str()),
                    "trough_cents": tl.trough.as_ref().map(|p| p.balance_cents),
                    "trough_date": tl.trough.as_ref().map(|p| p.date.as_str()),
                    "current_cents": tl.current_cents,
                    "amount_reliable": amount_reliable,
                    "history_starts": tl.earliest_txn_date,
                }));
            }

            let mut notes: Vec<String> = Vec::new();
            if !unreliable.is_empty() {
                notes.push(format!(
                    "Balance amounts for {} are NOT reliable: the account's history was imported \
                     behind a zero opening balance, so every figure is off by the same unknown \
                     amount. The DATES are correct. Report when the peak happened and say the \
                     dollar amount can't be pinned down until an opening or current balance is set.",
                    unreliable.join(", ")
                ));
            }
            if !skipped.is_empty() {
                notes.push(format!(
                    "Could not reconstruct {}. Say so and give the reason — do not substitute the \
                     current balance or leave the account out silently.",
                    skipped.join("; ")
                ));
            }
            notes.push(
                "History only reaches back to the earliest imported transaction (history_starts); \
                 a higher balance before that date would be invisible."
                    .to_string(),
            );

            Ok(json!({ "accounts": accounts, "note": notes.join(" ") }))
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

pub fn explain_metric() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "explain_metric"
        }
        fn description(&self) -> &str {
            "Explain how a headline financial metric is produced: its plain-English \
             definition, the inputs that fed it (with amounts), what was excluded, the \
             assumptions and time period, and any data-quality warnings. Use for 'how is \
             my savings rate calculated', 'why is my net worth what it is', 'what does \
             runway mean', or whenever a figure looks surprising and the user wants the \
             basis. The values come straight from the app's shared metrics layer — the \
             SAME numbers the dashboard shows — so report them as given and never \
             recompute. A value whose kind is 'withheld' means the app deliberately \
             declines to state it (too little history, or a per-person figure that isn't \
             meaningful); explain WHY from its warnings instead of inventing a number. \
             Pass `metric` to focus on one of: net_worth, avg_monthly_income, \
             avg_monthly_expense, monthly_surplus, savings_rate, emergency_fund_months, \
             runway_days; omit it to get all of them."
        }
        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "metric": {
                        "type": "string",
                        "description": "Optional metric key to focus on. Omit for every metric."
                    }
                }
            })
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let all = finsight_core::provenance::explain_financial_metrics(ctx.conn, None)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            let focus = args["metric"].as_str().map(str::trim).filter(|s| !s.is_empty());
            let metrics: Vec<Value> = all
                .into_iter()
                .filter(|e| focus.map_or(true, |k| e.key == k))
                .map(|e| serde_json::to_value(e).unwrap_or(Value::Null))
                .collect();
            let unknown = focus.is_some() && metrics.is_empty();
            Ok(json!({
                "metrics": metrics,
                "note": if unknown {
                    "No metric by that key. Valid keys: net_worth, avg_monthly_income, avg_monthly_expense, monthly_surplus, savings_rate, emergency_fund_months, runway_days. Call again with one of those, or omit `metric` for all.".to_string()
                } else {
                    "These figures come from the app's shared metrics layer — report them as-is, do not recompute. A value with kind 'withheld' means the app declines to state it; explain why from its warnings rather than inventing a number.".to_string()
                }
            }))
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
            "Get this month's income, expenses, and savings rate. Use for monthly income and cash-flow questions; for forward month-by-month projections use run_cashflow_timeline instead."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            let now = chrono::Utc::now();
            let month_start = now.format("%Y-%m-01").to_string();
            // Routed through the shared metrics layer (single-sourced, already
            // nets a settle_up inflow against expense and never counts it as
            // income) so the Copilot's month totals agree with Today.
            let (income, expense) = finsight_core::metrics::income_expense_since(
                ctx.conn,
                &month_start,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
                "SELECT c.label, SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                          WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                          ELSE 0 END) AS spent \
                 FROM transactions t JOIN categories c ON c.id = t.category_id \
                 WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ?1 \
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
            "Where the money goes over a window of months: top spending categories, top merchants, and per-month spend totals. Use for 'where am I spending the most' and overspending questions. The window defaults to the last 6 months, but the history often goes back years — the result includes data_range (earliest/latest transaction dates). ALWAYS widen `months` (up to 60) to cover the period asked about before concluding that data is missing; never say 'data only goes back N months' from a short window."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {"months": {"type": "integer", "default": 6, "description": "Whole months of history to analyze, ending this month. Widen (up to 60) for older periods."}, "limit": {"type": "integer", "default": 8}}})
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
                "SELECT COALESCE(c.label, 'Uncategorized') AS label, \
                        SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                 WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                 ELSE 0 END) AS spent \
                 FROM transactions t LEFT JOIN categories c ON c.id = t.category_id \
                 WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ?1 \
                 GROUP BY label ORDER BY spent DESC LIMIT ?2",
            )?;
            let top_categories: Vec<Value> = cat_stmt
                .query_map(rusqlite::params![start_str, limit], |r| {
                    Ok(json!({"category": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut merch_stmt = ctx.conn.prepare(
                "SELECT t.merchant_raw, \
                        SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                 WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                 ELSE 0 END) AS spent \
                 FROM transactions t \
                 WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ?1 \
                 GROUP BY t.merchant_raw ORDER BY spent DESC LIMIT ?2",
            )?;
            let top_merchants: Vec<Value> = merch_stmt
                .query_map(rusqlite::params![start_str, limit], |r| {
                    Ok(json!({"merchant": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            let mut month_stmt = ctx.conn.prepare(
                "SELECT substr(t.posted_at, 1, 7) AS ym, \
                        SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                 WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                 ELSE 0 END) AS spent \
                 FROM transactions t \
                 WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ?1 \
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

            // Full available history, so the model never mistakes a short window
            // for the extent of the data ("data only goes back 6 months").
            let (earliest, latest): (Option<String>, Option<String>) = ctx
                .conn
                .query_row(
                    "SELECT MIN(substr(posted_at,1,10)), MAX(substr(posted_at,1,10)) \
                     FROM transactions WHERE is_transfer = 0",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap_or((None, None));
            let window_note = match earliest.as_deref() {
                Some(e) if e < start_str.as_str() => format!(
                    "History goes back to {e}. This window only covers the last {months} month(s) — increase `months` (up to 60) to analyze earlier periods; do not conclude data is missing from this short window."
                ),
                _ => String::new(),
            };

            Ok(json!({
                "window_months": months,
                "window_start": start_str,
                "data_range": {"earliest": earliest, "latest": latest},
                "top_categories": top_categories,
                "top_merchants": top_merchants,
                "monthly": monthly,
                "total_spent_cents": total_spent_cents,
                "note": window_note,
            }))
        }
    }
    Arc::new(T)
}

pub fn get_member_spending() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_member_spending"
        }
        fn description(&self) -> &str {
            "Income and spending for ONE household member (e.g. a partner or family member) over a window of months. Joint accounts are split equally between their owners; household-shared (unassigned) accounts are excluded. Call with member omitted or member=\"list\" to see who is in the household FIRST. Use for 'what did <name> spend/earn last month', 'her savings rate', 'my spending vs theirs'. Never guess a person's numbers — if the member is unknown, say so."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{
                "member": {"type":"string","description":"Household member name (case-insensitive). Omit or \"list\" to list members."},
                "months": {"type":"integer","default":1,"description":"Whole calendar months back to include, ending this month."}
            }})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let members = finsight_core::repos::household::list_members(ctx.conn)?;
            let names: Vec<String> = members.iter().map(|m| m.name.clone()).collect();
            let requested = args["member"].as_str().unwrap_or("").trim().to_string();

            // Discovery / no-member: return the roster so the model can pick a
            // real member instead of inventing one.
            if requested.is_empty() || requested.eq_ignore_ascii_case("list") {
                return Ok(json!({
                    "household_members": names,
                    "note": if names.is_empty() {
                        "No household members are defined — every number in the app is for the whole household. Answer at the household level.".to_string()
                    } else {
                        format!("Call again with member set to one of: {}.", names.join(", "))
                    }
                }));
            }

            let Some(member) = members
                .iter()
                .find(|m| m.name.eq_ignore_ascii_case(&requested))
            else {
                return Ok(json!({
                    "error": "unknown_member",
                    "requested": requested,
                    "household_members": names,
                    "note": format!(
                        "No household member named \"{requested}\". Known members: {}. Do NOT report numbers for an unknown person.",
                        if names.is_empty() { "none".to_string() } else { names.join(", ") }
                    ),
                }));
            };

            let months = args["months"].as_i64().unwrap_or(1).clamp(1, 60);
            let now = chrono::Utc::now().date_naive();
            let start = {
                use chrono::Datelike;
                let total = now.year() * 12 + (now.month0() as i32) - (months as i32 - 1);
                let y = total.div_euclid(12);
                let m = total.rem_euclid(12) as u32 + 1;
                chrono::NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(now)
            };
            let start_str = start.format("%Y-%m-%d").to_string();

            // The SAME weighted metric the per-member screens use, so the
            // Copilot's number is literally the screen's number.
            let (income, expense) = finsight_core::metrics::income_expense_since_for(
                ctx.conn,
                &start_str,
                Some(member.id.as_str()),
            )?;
            let savings_rate = finsight_core::metrics::savings_rate_pct(income, expense);
            let balances =
                finsight_core::metrics::balance_breakdown_for(ctx.conn, Some(member.id.as_str()))?;

            // Ownership-weighted top spending categories for this member. The
            // weight subquery is the shared single source (metrics layer) so the
            // Copilot attributes exactly as the screens do.
            let mut cat_stmt = ctx.conn.prepare(&format!(
                "SELECT COALESCE(c.label, 'Uncategorized') AS label, \
                        CAST(ROUND(SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents * w.weight \
                                            WHEN t.amount_cents < 0 THEN -t.amount_cents * w.weight \
                                            ELSE 0 END)) AS INTEGER) AS spent \
                 FROM transactions t \
                 LEFT JOIN categories c ON c.id = t.category_id \
                 JOIN ({weight}) w ON w.account_id = t.account_id \
                 WHERE (t.amount_cents < 0 OR t.settle_up = 1) AND t.is_transfer = 0 AND t.posted_at >= ?2 \
                 GROUP BY label ORDER BY spent DESC LIMIT 8",
                weight = finsight_core::metrics::MEMBER_WEIGHT_SUBQUERY,
            ))?;
            let top_categories: Vec<Value> = cat_stmt
                .query_map(rusqlite::params![member.id, start_str], |r| {
                    Ok(json!({"category": r.get::<_, String>(0)?, "spent_cents": r.get::<_, i64>(1)?}))
                })?
                .filter_map(|r| r.ok())
                .collect();

            Ok(json!({
                "member": member.name,
                "window_months": months,
                "window_start": start_str,
                "income_cents": income,
                "spent_cents": expense,
                "net_cents": income - expense,
                "savings_rate_pct": savings_rate,
                "liquid_balance_cents": balances.liquid_cents,
                "owned_net_worth_cents": balances.net_worth_cents,
                "top_categories": top_categories,
                "note": "Joint accounts are split equally between owners; household-shared (unassigned) accounts are excluded from this member's figures.",
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
                        COALESCE((SELECT SUM(CASE WHEN t.settle_up = 1 THEN -t.amount_cents \
                                                   WHEN t.amount_cents < 0 THEN -t.amount_cents \
                                                   ELSE 0 END) FROM transactions t \
                                  WHERE t.category_id = b.category_id AND (t.amount_cents < 0 OR t.settle_up = 1) \
                                    AND t.is_transfer = 0 AND t.posted_at >= ?1), 0) AS spent \
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
            let items =
                detect_recurring(ctx.conn, 395).map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
            // Debt is a Credit/Loan-type Account with a negative balance, not
            // a separate liabilities-table row; "balance_cents" here is the
            // amount owed (positive), matching the old liabilities convention.
            let mut stmt = ctx.conn.prepare(
                "SELECT id, name, type, balance, apr_pct, limit_cents, min_payment_cents, payoff_date, promo_apr_expires_on, post_promo_apr_pct FROM (
                     SELECT a.id, a.name, a.type,
                            -COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC, CASE source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END LIMIT 1), 0) AS balance,
                            a.apr_pct, a.limit_cents, a.min_payment_cents, a.payoff_date,
                            a.promo_apr_expires_on, a.post_promo_apr_pct
                     FROM accounts a
                     WHERE a.archived_at IS NULL AND a.type IN ('Credit', 'Loan')
                 ) WHERE balance > 0
                 ORDER BY balance DESC"
            )?;
            let rows: Vec<Value> = stmt.query_map([], |r| {
                let account_type: String = r.get(2)?;
                let liability_type = if account_type == "Credit" { "credit-card" } else { "loan" };
                // `apr_pct` is the rate TODAY. Handing the model a promotional
                // 0% with no end date attached invites it to describe a balance
                // as free that is about to become expensive, so the expiry and
                // the rate it reverts to travel with it.
                Ok(json!({"id": r.get::<_, String>(0)?, "name": r.get::<_, String>(1)?, "liability_type": liability_type, "balance_cents": r.get::<_, i64>(3)?, "apr_pct": r.get::<_, Option<f64>>(4)?, "limit_cents": r.get::<_, Option<i64>>(5)?, "min_payment_cents": r.get::<_, Option<i64>>(6)?, "payoff_date": r.get::<_, Option<String>>(7)?, "promo_apr_expires_on": r.get::<_, Option<String>>(8)?, "post_promo_apr_pct": r.get::<_, Option<f64>>(9)?}))
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
            // Cap to keep payloads bounded even for large ranges.
            let limit = args["limit"].as_i64().unwrap_or(50).clamp(1, 500);
            let query = finsight_core::repos::transactions::SearchTxnQuery {
                merchant: args["merchant"].as_str().map(String::from),
                account: args["account"].as_str().map(String::from),
                start_date: args["start_date"].as_str().map(String::from),
                end_date: args["end_date"].as_str().map(String::from),
                min_amount_cents: args["min_amount_cents"].as_i64(),
                direction: args["direction"]
                    .as_str()
                    .filter(|d| *d != "any")
                    .map(String::from),
            };
            let rows: Vec<Value> =
                finsight_core::repos::transactions::search(ctx.conn, &query, limit)?
                    .into_iter()
                    .map(|r| {
                        json!({
                            "date": r.date,
                            "merchant": r.merchant,
                            "amount_cents": r.amount_cents,
                            "account": r.account,
                            "category": r.category
                        })
                    })
                    .collect();
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
            // Uncategorized means uncategorized SPENDING (expenses): income
            // inflows like payroll are legitimately left without a spending
            // category and must not be counted here — otherwise the count is
            // dominated by paycheck rows. Matches build_snapshot's definition.
            let pred = finsight_core::metrics::non_investment_txn_predicate("t");
            let total_uncategorized: i64 = ctx.conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM transactions t \
                     WHERE category_id IS NULL AND amount_cents < 0 AND is_transfer = 0 AND {pred}"
                ),
                [],
                |r| r.get(0),
            )?;
            let mut txn_stmt = ctx.conn.prepare(&format!(
                "SELECT t.id, t.merchant_raw, t.amount_cents, substr(t.posted_at,1,10), COALESCE(a.name,'Unknown account') \
                 FROM transactions t LEFT JOIN accounts a ON a.id = t.account_id \
                 WHERE t.category_id IS NULL AND t.amount_cents < 0 AND t.is_transfer = 0 AND {pred} \
                 ORDER BY t.posted_at DESC LIMIT ?1",
            ))?;
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
                "SELECT id, label, guidance FROM categories WHERE archived_at IS NULL ORDER BY label",
            )?;
            let available_categories: Vec<Value> = cat_stmt
                .query_map([], |r| {
                    let guidance: Option<String> = r.get(2)?;
                    let mut obj =
                        json!({"id": r.get::<_, String>(0)?, "label": r.get::<_, String>(1)?});
                    // Surface the user's per-category guidance so the model
                    // follows their intent when proposing categories.
                    if let Some(g) = guidance.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        obj["guidance"] = json!(g);
                    }
                    Ok(obj)
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
            // Routed through the shared metrics layer (single-sourced, already
            // nets a settle_up inflow against expense and never counts it as
            // income) so the projection agrees with Today and get_month_totals.
            let (income, expense) = finsight_core::metrics::income_expense_since(
                ctx.conn,
                &month_start,
            )
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
            "Get the full local finance snapshot for planning: liquid balances, cashflow, goals, debts, recurring bills, planned transactions, and data warnings. Any account with balance_known=false has an UNKNOWN balance (its balance_cents is a placeholder 0) — report it as unknown, never as $0, and exclude it from totals."
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
            // No explicit method means "use whatever school this user
            // subscribes to", not a hard-coded one.
            let preferred = finsight_core::metrics::philosophy(ctx.conn)
                .debt_strategy
                .as_method();
            let method = args["method"].as_str().unwrap_or(preferred);
            Ok(serde_json::to_value(finance::rank_debt_payoff(
                ctx.conn, method,
            )?)?)
        }
    }
    Arc::new(T)
}

/// Side-by-side of two payoff strategies over the same debts.
///
/// Exists because presenting one strategy as *the* strategy hides a tradeoff
/// the user is entitled to make. Every difference is computed in Rust — the
/// model must never do arithmetic on money.
/// Where the user stands with the people money has moved between.
///
/// Answers "am I up or down with this person, and by how much" from real legs
/// crossing the user's own accounts. Nothing is stored: the tab is recomputed
/// every time, so it cannot drift out of date the way a hand-maintained
/// balance would.
/// What the user's sinking funds require each month.
///
/// Separate from goal ETA because the question is inverted: a goal asks "when
/// will I get there at this rate", a sinking fund asks "what rate gets me
/// there by this date". The date is given, so the monthly figure is
/// arithmetic rather than a projection.
pub fn plan_sinking_funds() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "plan_sinking_funds"
        }
        fn description(&self) -> &str {
            "Required monthly contribution for each sinking fund — a known amount due on a known \
             date, like car insurance or property tax. Use for 'am I saving enough for X', \
             'what do my sinking funds cost me per month', or before allocating spare money, \
             since these are commitments against the same surplus goals compete for. Reports \
             shortfalls and overdue funds."
        }
        fn parameters(&self) -> Value {
            json!({"type": "object", "properties": {}})
        }
        fn execute(&self, ctx: &mut ToolContext, _args: Value) -> Result<Value> {
            Ok(serde_json::to_value(finance::plan_sinking_funds(ctx.conn)?)?)
        }
    }
    Arc::new(T)
}

pub fn get_counterparty_position() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "get_counterparty_position"
        }
        fn description(&self) -> &str {
            "Net position with the people money has moved between — who owes whom, and how much. \
             Use for 'does anyone owe me money', 'am I square with X', 'how much did I lend X'. \
             Pass `name` for one person, or omit it for everyone. Amounts are derived from real \
             transactions every time, so they are current by construction."
        }
        fn parameters(&self) -> Value {
            json!({"type":"object","properties":{"name":{"type":"string"}}})
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let name = args["name"].as_str().map(str::trim).filter(|s| !s.is_empty());
            let positions = match name {
                Some(n) => finsight_core::repos::transactions::counterparty_position(ctx.conn, n)?
                    .into_iter()
                    .collect::<Vec<_>>(),
                None => finsight_core::repos::transactions::list_counterparty_positions(ctx.conn)?,
            };
            let rows: Vec<Value> = positions
                .iter()
                .map(|p| {
                    json!({
                        "name": p.label,
                        "txn_count": p.txn_count,
                        "they_sent_me_cents": p.inflow_cents,
                        "i_sent_them_cents": p.outflow_cents,
                        "net_cents": p.net_cents,
                        "they_owe_me_cents": p.owed_to_user_cents(),
                        "i_owe_them_cents": p.owed_by_user_cents(),
                        "first_at": p.first_at,
                        "last_at": p.last_at,
                    })
                })
                .collect();
            Ok(json!({
                "counterparties": rows,
                // Distinguishes "nobody by that name" from "square with them",
                // which are different answers.
                "found": !rows.is_empty(),
                "searched_for": name,
            }))
        }
    }
    Arc::new(T)
}

pub fn compare_payoff_strategies() -> Arc<dyn Tool> {
    struct T;
    impl Tool for T {
        fn name(&self) -> &str {
            "compare_payoff_strategies"
        }
        fn description(&self) -> &str {
            "Compare two debt payoff strategies side by side — total interest, months to              debt-free, and when the first debt clears. Use for 'snowball vs avalanche',              'which payoff method is better', or to justify a hybrid order. Pass              custom_order (account ids) to model a hybrid plan: clear those first, then              optimise the rest by APR."
        }
        fn parameters(&self) -> Value {
            json!({
                "type": "object",
                "properties": {
                    "baseline_method": {"type": "string", "enum": ["avalanche", "snowball"], "default": "avalanche"},
                    "alternative_method": {"type": "string", "enum": ["avalanche", "snowball"], "default": "snowball"},
                    "custom_order": {"type": "array", "items": {"type": "string"}},
                    "extra_monthly_payment_cents": {"type": "integer", "default": 0}
                }
            })
        }
        fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value> {
            let baseline = args["baseline_method"].as_str().unwrap_or("avalanche");
            let alternative = args["alternative_method"].as_str().unwrap_or("snowball");
            let custom_order = args["custom_order"].as_array().map(|items| {
                items
                    .iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            });
            let extra = args["extra_monthly_payment_cents"].as_i64().unwrap_or(0);
            Ok(serde_json::to_value(finance::compare_payoff_strategies(
                ctx.conn,
                baseline,
                alternative,
                custom_order,
                extra,
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
            let preferred = finsight_core::metrics::philosophy(ctx.conn)
                .debt_strategy
                .as_method();
            let method = args["method"].as_str().unwrap_or(preferred);
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
            "Model one-, three-, and six-month emergency fund targets, gaps, liquidity runway, and time-to-target. Defaults the monthly savings rate to the current monthly surplus and returns an estimated completion date per target. Use for emergency-fund questions, 'when will my emergency fund be full', how-long-could-I-get-by-if-I-lost-my-income, and liquidity-runway questions. Report the target, current saved amount, the monthly contribution used, and the completion date."
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
            "Model whether a one-time purchase is affordable. Pass the purchase amount in cents (amount_cents). Weighs emergency cash, monthly surplus, obligations, and high-interest debt, and suggests wait/save alternatives. Be cautious: do not approve a purchase that would drop the user below their emergency floor or lean on high-APR debt. Use for 'can I afford X for $N' questions."
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


    /// The Copilot proposing a category for a transfer would write a category
    /// onto a row the whole app treats as an internal move — the invariant the
    /// categorizer, the metrics layer and the importer all hold. This tool is
    /// where a conversational flow first sees those rows, so it is where the
    /// exclusion has to bite.
    #[test]
    fn uncategorized_listing_never_offers_a_transfer() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('chk','Me','Bank','Checking','Everyday','USD','#fff',datetime('now'))", []).unwrap();
        // A genuine uncategorized expense.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at)              VALUES('t-real','chk','2026-05-01',-4200,'Corner Shop','cleared',datetime('now'))",
            [],
        )
        .unwrap();
        // A flagged transfer, otherwise identical.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_transfer,created_at)              VALUES('t-xfer','chk','2026-05-02',-50000,'INTERNET TRANSFER 123','cleared',1,datetime('now'))",
            [],
        )
        .unwrap();

        let out = call(&mut conn, list_uncategorized_transactions(), json!({}));
        let ids: Vec<String> = out["uncategorized"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap_or_default().to_string())
            .collect();

        assert!(ids.contains(&"t-real".to_string()), "the real expense should be listed");
        assert!(
            !ids.contains(&"t-xfer".to_string()),
            "a transfer must never be offered for categorization"
        );
        // The headline count has to agree with the rows, or the answer says
        // "12 need categorizing" and shows 11.
        assert_eq!(out["total_uncategorized"].as_i64(), Some(1));
    }

    /// Brokerage rows carry no transfer vocabulary in their merchant ("Buy
    /// ACME"), so they are excluded by account type rather than by keyword.
    /// Proposing "Shopping" for a stock purchase is the failure here.
    #[test]
    fn uncategorized_listing_never_offers_investment_activity() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('brk','Me','Broker','Investment','TFSA','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('chk','Me','Bank','Checking','Everyday','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,activity_type,created_at)              VALUES('t-trade','brk','2026-05-01',-100000,'Buy ACME','cleared','Trade',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at)              VALUES('t-spend','chk','2026-05-02',-4200,'Corner Shop','cleared',datetime('now'))",
            [],
        )
        .unwrap();

        let out = call(&mut conn, list_uncategorized_transactions(), json!({}));
        let ids: Vec<String> = out["uncategorized"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap_or_default().to_string())
            .collect();

        assert_eq!(ids, vec!["t-spend".to_string()]);
    }

    /// Income is legitimately uncategorized — a paycheck has no spending
    /// category. Offering payroll rows would bury the real work.
    #[test]
    fn uncategorized_listing_ignores_income() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('chk','Me','Bank','Checking','Everyday','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at)              VALUES('t-pay','chk','2026-05-01',300000,'Payroll','cleared',datetime('now'))",
            [],
        )
        .unwrap();

        let out = call(&mut conn, list_uncategorized_transactions(), json!({}));
        assert!(out["uncategorized"].as_array().unwrap().is_empty());
        assert_eq!(out["total_uncategorized"].as_i64(), Some(0));
    }

    fn seed_txns(conn: &mut Connection) {
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('chk','Me','Bank','Checking','Everyday Checking','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('amex','Me','Amex','Credit','Amex Card','USD','#111',datetime('now'))", []).unwrap();
        // (account, date, amount_cents, merchant)
        let rows = [
            ("chk", "2026-01-15", -9_999, "Costco"), // Jan, over $60 expense
            ("chk", "2026-02-10", -4_200, "Tim Hortons"), // under $60
            ("amex", "2026-03-05", -12_050, "Best Buy"), // over $60
            ("amex", "2026-06-28", -6_100, "Uber"),  // June, just over $60
            ("chk", "2026-07-02", -20_000, "Rent"),  // OUT of range (July)
            ("chk", "2026-03-01", 300_000, "Payroll"), // income, over $60 but positive
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

    /// A savings account with a REAL opening anchor and a clear rise-then-fall,
    /// so the peak sits on a day no stored snapshot records.
    fn seed_savings_with_a_peak(conn: &mut Connection) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('sav','Me','Bank','Savings','Car Savings','USD','#fff','2023-12-01T00:00:00+00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) \
             VALUES('sav','2023-12-01',100000,'seed')",
            [],
        )
        .unwrap();
        for (date, amt) in [
            ("2024-02-01", 500_000),
            ("2024-05-01", 400_000),  // peak: $10,000
            ("2024-08-01", -700_000), // back down to $3,000
        ] {
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
                 VALUES(hex(randomblob(16)),'sav',?1,?2,'Transfer','cleared',datetime('now'))",
                rusqlite::params![date, amt],
            )
            .unwrap();
        }
    }

    #[test]
    fn balance_history_tool_answers_the_peak_question_by_partial_name() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_savings_with_a_peak(&mut conn);

        // "savings" is how someone would actually refer to "Car Savings".
        let out = call(
            &mut conn,
            get_account_balance_history(),
            json!({"account": "savings"}),
        );

        let acct = &out["accounts"][0];
        assert_eq!(acct["account"], "Car Savings");
        assert_eq!(acct["peak_cents"], 1_000_000);
        assert_eq!(acct["peak_date"], "2024-05-01");
        assert_eq!(acct["current_cents"], 300_000);
        assert_eq!(acct["amount_reliable"], true);
    }

    /// The caveat has to reach the model as data, not be left for it to infer:
    /// an account whose history was imported behind a zero opening has correct
    /// DATES and meaningless AMOUNTS.
    #[test]
    fn balance_history_tool_marks_an_unanchored_account_unreliable() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_txns(&mut conn); // accounts created with no opening balance row

        let out = call(
            &mut conn,
            get_account_balance_history(),
            json!({"account": "Everyday Checking"}),
        );

        let acct = &out["accounts"][0];
        assert_eq!(acct["amount_reliable"], false);
        assert!(acct["peak_date"].is_string(), "the date is still reported");
        let note = out["note"].as_str().unwrap();
        assert!(note.contains("NOT reliable"), "note was: {note}");
        assert!(note.contains("DATES are correct"), "note was: {note}");
    }

    /// A miss must tell the model to ask rather than silently returning nothing,
    /// which reads as "you have no such account".
    #[test]
    fn balance_history_tool_tells_the_model_to_ask_when_no_account_matches() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_savings_with_a_peak(&mut conn);

        let out = call(
            &mut conn,
            get_account_balance_history(),
            json!({"account": "chequing"}),
        );

        assert_eq!(out["accounts"].as_array().unwrap().len(), 0);
        assert!(out["note"].as_str().unwrap().contains("do not guess"));
    }

    /// Investment accounts hold market value, not summed cash flow — the tool has
    /// to say so rather than quietly omitting them.
    #[test]
    fn balance_history_tool_explains_why_investment_accounts_are_skipped() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('inv','Me','Broker','Investment','TFSA','USD','#fff',datetime('now'))",
            [],
        )
        .unwrap();

        let out = call(
            &mut conn,
            get_account_balance_history(),
            json!({"account": "TFSA"}),
        );

        assert_eq!(out["accounts"].as_array().unwrap().len(), 0);
        assert!(out["note"].as_str().unwrap().contains("market value"));
    }

    /// Credit balances are stored negative, so "when did I owe the most" is the
    /// TROUGH, not the peak. The account type has to reach the model as data or
    /// it can only guess which end of the range answers the question.
    #[test]
    fn balance_history_tool_surfaces_account_type_so_debt_reads_correctly() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES('cc','Me','Amex','Credit','Amex Card','USD','#fff','2024-01-01T00:00:00+00:00')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) \
             VALUES('cc','2024-01-01',0,'seed')",
            [],
        )
        .unwrap();
        for (date, amt) in [
            ("2024-02-01", -100_000),
            ("2024-03-01", -200_000), // owes the most here: -$3,000
            ("2024-04-01", 250_000),  // paid most of it off
        ] {
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
                 VALUES(hex(randomblob(16)),'cc',?1,?2,'Card','cleared',datetime('now'))",
                rusqlite::params![date, amt],
            )
            .unwrap();
        }

        let out = call(
            &mut conn,
            get_account_balance_history(),
            json!({"account": "Amex"}),
        );

        let acct = &out["accounts"][0];
        assert_eq!(acct["account_type"], "Credit");
        assert_eq!(acct["trough_cents"], -300_000);
        assert_eq!(acct["trough_date"], "2024-03-01");
        // A card that genuinely opened at zero is anchored, not assumed.
        assert_eq!(acct["amount_reliable"], true);
    }

    fn seed_household(conn: &mut Connection) {
        use finsight_core::repos::household;
        let alice = household::create_member(conn, "Alice", None).unwrap();
        let bob = household::create_member(conn, "Bob", None).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a_sole','Alice','Bank','Checking','A','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('joint','Alice & Bob','Bank','Savings','J','USD','#fff',datetime('now'))", []).unwrap();
        household::set_account_owners(conn, "a_sole", &[alice.id.clone()]).unwrap();
        household::set_account_owners(conn, "joint", &[alice.id.clone(), bob.id.clone()]).unwrap();
        // Alice sole: -$100. Joint: -$40 (split → -$20 each). Dates well in the past
        // so any months-window that reaches back far enough includes them.
        for (acct, date, amt, merch) in [
            ("a_sole", "2026-07-05", -10_000, "Costco"),
            ("joint", "2026-07-06", -4_000, "Dining"),
        ] {
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
                 VALUES(hex(randomblob(16)),?1,?2,?3,?4,'cleared',datetime('now'))",
                rusqlite::params![acct, date, amt, merch],
            )
            .unwrap();
        }
    }

    #[test]
    fn member_spending_lists_members_and_weights_joint_accounts() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_household(&mut conn);

        // Discovery: no member → roster, no fabricated numbers.
        let list = call(&mut conn, get_member_spending(), json!({"member": "list"}));
        let names: Vec<String> = list["household_members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"Alice".to_string()) && names.contains(&"Bob".to_string()));
        assert!(list.get("spent_cents").is_none());

        // Alice: sole $100 + half of joint $40 = $120. (months=60 so the window
        // reaches the seeded dates regardless of the wall clock.)
        let alice = call(&mut conn, get_member_spending(), json!({"member": "alice", "months": 60}));
        assert_eq!(alice["member"], "Alice");
        assert_eq!(alice["spent_cents"], 12_000, "sole 100 + half-joint 20");

        // Bob: only half of the joint account = $20.
        let bob = call(&mut conn, get_member_spending(), json!({"member": "Bob", "months": 60}));
        assert_eq!(bob["spent_cents"], 2_000);

        // Unknown member → explicit error, never a guessed number.
        let unknown = call(&mut conn, get_member_spending(), json!({"member": "Carol"}));
        assert_eq!(unknown["error"], "unknown_member");
        assert!(unknown.get("spent_cents").is_none());
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
        let merchants: Vec<&str> = txns
            .iter()
            .map(|t| t["merchant"].as_str().unwrap())
            .collect();
        assert!(merchants.contains(&"Costco"));
        assert!(merchants.contains(&"Best Buy"));
        assert!(merchants.contains(&"Uber"));
        assert!(!merchants.contains(&"Tim Hortons"));
        assert!(!merchants.contains(&"Rent"));
        assert!(!merchants.contains(&"Payroll"));
        // Rows carry account + category; total is grounded.
        assert!(txns
            .iter()
            .all(|t| t["account"].is_string() && t["category"].is_string()));
        assert_eq!(
            out["total_abs_cents"].as_i64().unwrap(),
            9_999 + 12_050 + 6_100
        );
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
        assert!(txns
            .iter()
            .all(|t| t["account"].as_str().unwrap() == "Amex Card"));
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
