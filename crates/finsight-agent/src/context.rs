use chrono::{Datelike, Duration, NaiveDate, Utc};
use finsight_core::{forecast, repos::goals};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinancialContext {
    pub generated_at: String,
    pub cashflow: CashflowContext,
    pub budget: BudgetContext,
    pub goals: Vec<GoalContextItem>,
    pub transactions: TransactionContext,
    pub memory: Vec<MemoryItem>,
    pub wellness: WellnessContext,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WellnessContext {
    /// Months of average expenses covered by current total balance (Ramsey emergency fund gauge)
    pub emergency_fund_months: f64,
    /// Sum of remaining balances across all active debt-payoff goals (Ramsey snowball)
    pub total_debt_cents: i64,
    /// Ordered list of debt payoff goals by remaining balance ascending (Ramsey snowball order)
    pub debt_snowball: Vec<DebtSnowballItem>,
    /// Savings rate direction vs. 30 days prior: "improving" | "stable" | "declining"
    pub savings_rate_trend: String,
    /// Whether savings rate meets Babylon's 10% minimum
    pub meets_pay_yourself_first: bool,
    /// Savings accounts and their APY for cash-efficiency advice
    pub savings_accounts: Vec<SavingsAccountItem>,
    /// Detailed loan records for payoff progress and history-aware advice
    pub loans: Vec<LoanDetailItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CashflowContext {
    pub total_balance_cents: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub net_monthly_cents: i64,
    pub savings_rate_pct: i64,
    pub runway_days: i64,
    pub this_month_income_cents: i64,
    pub this_month_expense_cents: i64,
    pub upcoming_bills: Vec<UpcomingBill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpcomingBill {
    pub label: String,
    pub amount_cents: i64,
    pub due_days_from_now: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetContext {
    pub month: String,
    pub total_budget_cents: i64,
    pub total_spent_cents: i64,
    pub to_budget_cents: i64,
    pub overages: Vec<BudgetOverage>,
    pub near_limit: Vec<BudgetNearLimit>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetOverage {
    pub category_label: String,
    pub budget_cents: i64,
    pub spent_cents: i64,
    pub overage_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BudgetNearLimit {
    pub category_label: String,
    pub budget_cents: i64,
    pub spent_cents: i64,
    pub pct_used: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GoalContextItem {
    pub id: String,
    pub name: String,
    pub purpose: Option<String>,
    pub target_cents: i64,
    pub current_cents: i64,
    pub monthly_cents: i64,
    pub pct_complete: i64,
    pub months_to_goal: Option<i64>,
    pub is_on_track: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TransactionContext {
    pub total_count: i64,
    pub uncategorized_count: i64,
    pub anomaly_count: i64,
    pub reimbursable_count: i64,
    pub top_merchants_this_month: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryItem {
    pub kind: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DebtSnowballItem {
    pub name: String,
    pub remaining_cents: i64,
    pub monthly_cents: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SavingsAccountItem {
    pub name: String,
    pub balance_cents: i64,
    pub apy_pct: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LoanDetailItem {
    pub name: String,
    pub balance_cents: i64,
    pub original_balance_cents: Option<i64>,
    pub apr_pct: Option<f64>,
    pub started_at: Option<String>,
    pub paid_down_pct: Option<i64>,
}

impl FinancialContext {
    pub fn to_prompt_string(&self) -> String {
        let mut lines = vec![
            format!("Generated at: {}", self.generated_at),
            "1. CASHFLOW".to_string(),
            format!(
                "   - Total balance: {}",
                fmt_money(self.cashflow.total_balance_cents)
            ),
            format!(
                "   - Monthly income (90-day avg): {}",
                fmt_money(self.cashflow.avg_monthly_income_cents)
            ),
            format!(
                "   - Monthly expenses (90-day avg): {}",
                fmt_money(self.cashflow.avg_monthly_expense_cents)
            ),
            format!(
                "   - Net monthly: {}",
                fmt_money(self.cashflow.net_monthly_cents)
            ),
            format!("   - Savings rate: {}%", self.cashflow.savings_rate_pct),
            format!("   - Estimated runway: {} days", self.cashflow.runway_days),
            format!(
                "   - This month: earned {}, spent {}",
                fmt_money(self.cashflow.this_month_income_cents),
                fmt_money(self.cashflow.this_month_expense_cents)
            ),
            "   - Upcoming bills:".to_string(),
        ];
        if self.cashflow.upcoming_bills.is_empty() {
            lines.push("     • none".to_string());
        } else {
            for bill in &self.cashflow.upcoming_bills {
                lines.push(format!(
                    "     • {} — {} due in {} days",
                    bill.label,
                    fmt_money(bill.amount_cents),
                    bill.due_days_from_now
                ));
            }
        }

        lines.extend([
            format!("2. BUDGET ({})", self.budget.month),
            format!(
                "   - Total budget: {} | Spent: {} | To-budget: {}",
                fmt_money(self.budget.total_budget_cents),
                fmt_money(self.budget.total_spent_cents),
                fmt_money(self.budget.to_budget_cents)
            ),
            "   - Over-budget categories:".to_string(),
        ]);
        if self.budget.overages.is_empty() {
            lines.push("     • none".to_string());
        } else {
            for over in &self.budget.overages {
                lines.push(format!(
                    "     • {} — spent {} vs budget {} (over by {})",
                    over.category_label,
                    fmt_money(over.spent_cents),
                    fmt_money(over.budget_cents),
                    fmt_money(over.overage_cents)
                ));
            }
        }

        lines.push("   - Near-limit categories:".to_string());
        if self.budget.near_limit.is_empty() {
            lines.push("     • none".to_string());
        } else {
            for item in &self.budget.near_limit {
                lines.push(format!(
                    "     • {} — {} of {} used ({}%)",
                    item.category_label,
                    fmt_money(item.spent_cents),
                    fmt_money(item.budget_cents),
                    item.pct_used
                ));
            }
        }

        lines.push("3. GOALS".to_string());
        if self.goals.is_empty() {
            lines.push("   - none".to_string());
        } else {
            for goal in &self.goals {
                let eta = goal
                    .months_to_goal
                    .map(|m| format!("{m} months"))
                    .unwrap_or_else(|| "unknown".to_string());
                let purpose_str = goal
                    .purpose
                    .as_deref()
                    .map(|p| format!(", why: \"{p}\""))
                    .unwrap_or_default();
                lines.push(format!(
                    "   - {} [{}]: target {}, current {}, monthly {}, complete {}%, ETA {}, on-track {}{}",
                    goal.name,
                    goal.id,
                    fmt_money(goal.target_cents),
                    fmt_money(goal.current_cents),
                    fmt_money(goal.monthly_cents),
                    goal.pct_complete,
                    eta,
                    goal.is_on_track,
                    purpose_str
                ));
            }
        }

        lines.extend([
            "4. TRANSACTIONS".to_string(),
            format!(
                "   - Total: {} | Uncategorized: {} | Anomalies: {} | Reimbursable: {}",
                self.transactions.total_count,
                self.transactions.uncategorized_count,
                self.transactions.anomaly_count,
                self.transactions.reimbursable_count
            ),
            "   - Top merchants this month:".to_string(),
        ]);
        if self.transactions.top_merchants_this_month.is_empty() {
            lines.push("     • none".to_string());
        } else {
            for (merchant, total) in &self.transactions.top_merchants_this_month {
                lines.push(format!("     • {} — {}", merchant, fmt_money(*total)));
            }
        }

        lines.push("5. MEMORY".to_string());
        if self.memory.is_empty() {
            lines.push("   - none".to_string());
        } else {
            for item in &self.memory {
                lines.push(format!("   - [{}] {}", item.kind, item.content));
            }
        }

        lines.extend([
            "6. FINANCIAL WELLNESS".to_string(),
            format!(
                "   - Emergency fund coverage: {:.1} months of expenses (guideline: ≥3 months)",
                self.wellness.emergency_fund_months
            ),
            format!(
                "   - Total remaining debt (debt-payoff goals): {}",
                fmt_money(self.wellness.total_debt_cents)
            ),
            format!(
                "   - Savings rate trend (vs. 30 days prior): {}",
                self.wellness.savings_rate_trend
            ),
            format!(
                "   - Meets 'pay yourself first' (≥10% savings): {}",
                if self.wellness.meets_pay_yourself_first {
                    "YES"
                } else {
                    "NO — savings rate is below 10%"
                }
            ),
        ]);
        if !self.wellness.debt_snowball.is_empty() {
            lines.push(
                "   - Debt snowball order (smallest to largest — Ramsey method):".to_string(),
            );
            for (i, debt) in self.wellness.debt_snowball.iter().enumerate() {
                lines.push(format!(
                    "     {}. {} — {} remaining, {} /month",
                    i + 1,
                    debt.name,
                    fmt_money(debt.remaining_cents),
                    fmt_money(debt.monthly_cents)
                ));
            }
        }
        if !self.wellness.savings_accounts.is_empty() {
            lines.push("   - Savings accounts:".to_string());
            for a in &self.wellness.savings_accounts {
                let apy = a
                    .apy_pct
                    .map(|r| format!("{r}% APY"))
                    .unwrap_or_else(|| "no APY recorded".to_string());
                lines.push(format!(
                    "     • {} — {}, {}",
                    a.name,
                    fmt_money(a.balance_cents),
                    apy
                ));
            }
        }
        if !self.wellness.loans.is_empty() {
            lines.push("   - Loan history:".to_string());
            for l in &self.wellness.loans {
                let started = l.started_at.as_deref().unwrap_or("start date not recorded");
                let progress = l
                    .paid_down_pct
                    .map(|p| format!("{p}% paid down"))
                    .unwrap_or_else(|| "progress not tracked".to_string());
                let apr = l
                    .apr_pct
                    .map(|r| format!("{r}% APR"))
                    .unwrap_or_else(|| "APR not recorded".to_string());
                lines.push(format!(
                    "     • {} — {}, original {}, current {}, {}, started {}",
                    l.name,
                    apr,
                    fmt_money(l.original_balance_cents.unwrap_or(l.balance_cents)),
                    fmt_money(l.balance_cents),
                    progress,
                    started
                ));
            }
        }

        lines.join("\n")
    }
}

pub fn build_context(conn: &mut Connection) -> FinancialContext {
    let now = Utc::now();
    let month = now.format("%Y-%m").to_string();
    let month_start = now.format("%Y-%m-01").to_string();
    let rolling_cutoff = (now - Duration::days(90)).to_rfc3339();

    let total_balance_cents = latest_total_balance(conn);
    let (rolling_income_total, rolling_expense_total) =
        income_and_expense_since(conn, &rolling_cutoff);
    let avg_monthly_income_cents = rolling_income_total / 3;
    let avg_monthly_expense_cents = rolling_expense_total / 3;
    let net_monthly_cents = avg_monthly_income_cents - avg_monthly_expense_cents;
    let savings_rate_pct = if avg_monthly_income_cents > 0 {
        ((net_monthly_cents.max(0) * 100) / avg_monthly_income_cents).clamp(0, 100)
    } else {
        0
    };
    let runway_days =
        forecast::runway_days(total_balance_cents, avg_monthly_expense_cents.max(0), 30);
    let (this_month_income_cents, this_month_expense_cents) =
        income_and_expense_since(conn, &month_start);

    let (total_budget_cents, overages, near_limit) = budget_details(conn, &month, &month_start);
    let total_spent_cents: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0)
             FROM transactions
             WHERE posted_at >= ?1",
            params![month_start],
            |r| r.get(0),
        )
        .unwrap_or(0);

    FinancialContext {
        generated_at: now.to_rfc3339(),
        cashflow: CashflowContext {
            total_balance_cents,
            avg_monthly_income_cents,
            avg_monthly_expense_cents,
            net_monthly_cents,
            savings_rate_pct,
            runway_days,
            this_month_income_cents,
            this_month_expense_cents,
            upcoming_bills: upcoming_bills(conn),
        },
        budget: BudgetContext {
            month,
            total_budget_cents,
            total_spent_cents,
            to_budget_cents: this_month_income_cents - total_budget_cents,
            overages,
            near_limit,
        },
        goals: goal_context(conn, now.date_naive()),
        transactions: transaction_context(conn, &month_start),
        memory: recent_memory(conn),
        wellness: wellness_context(conn, avg_monthly_expense_cents, savings_rate_pct),
    }
}

fn latest_total_balance(conn: &mut Connection) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(COALESCE(
             (SELECT balance_cents
              FROM account_balances b
              WHERE b.account_id = a.id
              ORDER BY b.as_of_date DESC
              LIMIT 1),
             0
         )), 0)
         FROM accounts a
         WHERE a.archived_at IS NULL",
        [],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

fn income_and_expense_since(conn: &mut Connection, cutoff: &str) -> (i64, i64) {
    conn.query_row(
        "SELECT
            COALESCE(SUM(CASE WHEN amount_cents > 0 THEN amount_cents ELSE 0 END), 0),
            COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0)
         FROM transactions
         WHERE posted_at >= ?1",
        params![cutoff],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap_or((0, 0))
}

fn budget_details(
    conn: &mut Connection,
    month: &str,
    month_start: &str,
) -> (i64, Vec<BudgetOverage>, Vec<BudgetNearLimit>) {
    let total_budget_cents = conn
        .query_row(
            "SELECT COALESCE(SUM(amount_cents), 0) FROM budgets WHERE month = ?1",
            params![month],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let mut overages = Vec::new();
    let mut near_limit = Vec::new();
    let mut stmt = match conn.prepare(
        "WITH actuals AS (
            SELECT category_id,
                   COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0) AS spent_cents
            FROM transactions
            WHERE posted_at >= ?2
            GROUP BY category_id
         )
         SELECT c.label,
                b.amount_cents,
                COALESCE(a.spent_cents, 0) AS spent_cents
         FROM budgets b
         JOIN categories c ON c.id = b.category_id
         LEFT JOIN actuals a ON a.category_id = b.category_id
         WHERE b.month = ?1
         ORDER BY spent_cents DESC, c.label ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return (total_budget_cents, overages, near_limit),
    };

    let rows = match stmt.query_map(params![month, month_start], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return (total_budget_cents, overages, near_limit),
    };

    for row in rows.flatten() {
        let (category_label, budget_cents, spent_cents) = row;
        if budget_cents > 0 && spent_cents > budget_cents {
            overages.push(BudgetOverage {
                category_label: category_label.clone(),
                budget_cents,
                spent_cents,
                overage_cents: spent_cents - budget_cents,
            });
        } else if budget_cents > 0 {
            let pct_used = (spent_cents * 100) / budget_cents;
            if pct_used >= 80 {
                near_limit.push(BudgetNearLimit {
                    category_label,
                    budget_cents,
                    spent_cents,
                    pct_used,
                });
            }
        }
    }

    (total_budget_cents, overages, near_limit)
}

fn goal_context(conn: &mut Connection, today: NaiveDate) -> Vec<GoalContextItem> {
    goals::list(conn)
        .unwrap_or_default()
        .into_iter()
        .map(|goal| {
            let remaining = (goal.target_cents - goal.current_cents).max(0);
            let pct_complete = if goal.target_cents > 0 {
                ((goal.current_cents.max(0) * 100) / goal.target_cents).clamp(0, 100)
            } else if goal.current_cents > 0 {
                100
            } else {
                0
            };
            let months_to_goal = if goal.monthly_cents > 0 && remaining > 0 {
                Some((remaining + goal.monthly_cents - 1) / goal.monthly_cents)
            } else if remaining == 0 {
                Some(0)
            } else {
                None
            };
            let is_on_track = goal
                .target_date
                .as_deref()
                .and_then(parse_date)
                .map(|target_date| {
                    if remaining == 0 {
                        return true;
                    }
                    let Some(eta_months) = months_to_goal else {
                        return false;
                    };
                    eta_months <= months_until(today, target_date)
                })
                .unwrap_or(true);

            GoalContextItem {
                id: goal.id,
                name: goal.name,
                purpose: goal.purpose,
                target_cents: goal.target_cents,
                current_cents: goal.current_cents,
                monthly_cents: goal.monthly_cents,
                pct_complete,
                months_to_goal,
                is_on_track,
            }
        })
        .collect()
}

fn transaction_context(conn: &mut Connection, month_start: &str) -> TransactionContext {
    let counts = conn
        .query_row(
            "SELECT
                COUNT(*),
                COALESCE(SUM(CASE WHEN category_id IS NULL THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_anomaly = 1 THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN is_reimbursable = 1 THEN 1 ELSE 0 END), 0)
             FROM transactions",
            [],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, i64>(3)?,
                ))
            },
        )
        .unwrap_or((0, 0, 0, 0));

    let mut top_merchants_this_month = Vec::new();
    if let Ok(mut stmt) = conn.prepare(
        "SELECT merchant_raw, COALESCE(SUM(ABS(amount_cents)), 0) AS total_abs_cents
         FROM transactions
         WHERE posted_at >= ?1
         GROUP BY merchant_raw
         ORDER BY total_abs_cents DESC, merchant_raw ASC
         LIMIT 5",
    ) {
        if let Ok(rows) = stmt.query_map(params![month_start], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
        }) {
            top_merchants_this_month = rows.flatten().collect();
        }
    }

    TransactionContext {
        total_count: counts.0,
        uncategorized_count: counts.1,
        anomaly_count: counts.2,
        reimbursable_count: counts.3,
        top_merchants_this_month,
    }
}

fn recent_memory(conn: &mut Connection) -> Vec<MemoryItem> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT kind, description, merchant_key
         FROM agent_memory
         ORDER BY created_at DESC
         LIMIT 10",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return out,
    };
    let rows = match stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return out,
    };

    for row in rows.flatten() {
        let (kind, description, merchant_key) = row;
        let mapped_kind = match kind.as_str() {
            "correction" => "merchant_correction",
            other => other,
        }
        .to_string();
        let content = if kind == "correction" {
            if description.contains("→") || description.contains("->") {
                format!("{description} (user correction)")
            } else if let Some(merchant_key) = merchant_key {
                format!("{merchant_key}: {description} (user correction)")
            } else {
                format!("{description} (user correction)")
            }
        } else {
            description
        };
        out.push(MemoryItem {
            kind: mapped_kind,
            content,
        });
    }
    out
}

fn upcoming_bills(conn: &mut Connection) -> Vec<UpcomingBill> {
    let now = Utc::now().date_naive();
    let cutoff = (Utc::now() - Duration::days(395))
        .format("%Y-%m-%d")
        .to_string();
    let mut stmt = match conn.prepare(
        "WITH dated AS (
           SELECT t.merchant_raw,
                  date(t.posted_at) AS d,
                  t.amount_cents,
                  LAG(date(t.posted_at)) OVER (
                    PARTITION BY t.merchant_raw
                    ORDER BY t.posted_at
                  ) AS prev_d
           FROM transactions t
           WHERE t.posted_at >= ?1
         ),
         gaps AS (
           SELECT merchant_raw, d, amount_cents,
                  julianday(d) - julianday(prev_d) AS gap
           FROM dated
           WHERE prev_d IS NOT NULL
         ),
         agg AS (
           SELECT merchant_raw,
                  AVG(gap) AS avg_gap,
                  COUNT(*) AS occurrences,
                  MAX(d) AS last_seen,
                  MAX(amount_cents) AS last_amount
           FROM gaps
           WHERE gap BETWEEN 5 AND 400
           GROUP BY merchant_raw
           HAVING occurrences >= 2 AND AVG(gap) < 400 AND MAX(amount_cents) < 0
         )
         SELECT merchant_raw, avg_gap, last_seen, last_amount
         FROM agg
         ORDER BY last_seen ASC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };

    let rows = match stmt.query_map(params![cutoff], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, f64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, i64>(3)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return Vec::new(),
    };

    let mut out = Vec::new();
    for row in rows.flatten() {
        let (label, avg_gap, last_seen, last_amount) = row;
        let Some(last_seen_date) = parse_date(&last_seen) else {
            continue;
        };
        let next_due = last_seen_date + Duration::days(avg_gap.round() as i64);
        let due_days = (next_due - now).num_days();
        if (0..=30).contains(&due_days) {
            out.push(UpcomingBill {
                label,
                amount_cents: last_amount.abs(),
                due_days_from_now: due_days,
            });
        }
    }
    out.sort_by_key(|bill| bill.due_days_from_now);
    out.truncate(8);
    out
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .or_else(|| {
            chrono::DateTime::parse_from_rfc3339(value)
                .ok()
                .map(|dt| dt.date_naive())
        })
}

fn months_until(from: NaiveDate, to: NaiveDate) -> i64 {
    if to <= from {
        return 0;
    }
    let mut months =
        ((to.year() - from.year()) as i64 * 12) + to.month() as i64 - from.month() as i64;
    if to.day() > from.day() {
        months += 1;
    }
    months.max(0)
}

fn fmt_money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}${:.2}", cents.abs() as f64 / 100.0)
}

fn wellness_context(
    conn: &mut Connection,
    avg_monthly_expense_cents: i64,
    savings_rate_pct: i64,
) -> WellnessContext {
    // Emergency fund months: total liquid balance / avg monthly expenses
    let total_balance: i64 = conn
        .query_row(
            "SELECT COALESCE(SUM(COALESCE(
                 (SELECT balance_cents FROM account_balances b
                  WHERE b.account_id = a.id ORDER BY b.as_of_date DESC LIMIT 1), 0
             )), 0) FROM accounts a WHERE a.archived_at IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    let emergency_fund_months = if avg_monthly_expense_cents > 0 {
        (total_balance.max(0) as f64 / avg_monthly_expense_cents as f64).min(24.0)
    } else {
        0.0
    };

    // Debt snowball: active debt-payoff goals ordered by remaining balance ascending
    let mut debt_snowball: Vec<DebtSnowballItem> = Vec::new();
    let mut total_debt_cents: i64 = 0;
    if let Ok(mut stmt) = conn.prepare(
        "SELECT name, target_cents, current_cents, monthly_cents
         FROM goals
         WHERE goal_type = 'debt-payoff'
           AND archived_at IS NULL
           AND (target_cents - current_cents) > 0
         ORDER BY (target_cents - current_cents) ASC",
    ) {
        if let Ok(rows) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        }) {
            for row in rows.flatten() {
                let (name, target, current, monthly) = row;
                let remaining = (target - current).max(0);
                total_debt_cents += remaining;
                debt_snowball.push(DebtSnowballItem {
                    name,
                    remaining_cents: remaining,
                    monthly_cents: monthly,
                });
            }
        }
    }

    // Savings rate trend: compare current 30-day rate vs. 31-60 day rate
    let now = Utc::now();
    let cut30 = (now - Duration::days(30)).to_rfc3339();
    let cut60 = (now - Duration::days(60)).to_rfc3339();

    let rate_for = |cutoff_start: &str, cutoff_end: &str| -> i64 {
        let (inc, exp): (i64, i64) = conn
            .query_row(
                "SELECT
                   COALESCE(SUM(CASE WHEN amount_cents > 0 THEN amount_cents ELSE 0 END), 0),
                   COALESCE(SUM(CASE WHEN amount_cents < 0 THEN -amount_cents ELSE 0 END), 0)
                 FROM transactions
                 WHERE posted_at >= ?1 AND posted_at < ?2",
                params![cutoff_start, cutoff_end],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap_or((0, 0));
        if inc > 0 {
            (((inc - exp).max(0) * 100) / inc).clamp(0, 100)
        } else {
            0
        }
    };

    let rate_recent = rate_for(&cut30, &now.to_rfc3339());
    let rate_prior = rate_for(&cut60, &cut30);
    let savings_rate_trend = if rate_prior == 0 || (rate_recent - rate_prior).abs() <= 2 {
        "stable".to_string()
    } else if rate_recent > rate_prior {
        "improving".to_string()
    } else {
        "declining".to_string()
    };

    let savings_accounts = savings_account_context(conn);
    let loans = loan_context(conn);

    WellnessContext {
        emergency_fund_months,
        total_debt_cents,
        debt_snowball,
        savings_rate_trend,
        meets_pay_yourself_first: savings_rate_pct >= 10,
        savings_accounts,
        loans,
    }
}

fn savings_account_context(conn: &mut Connection) -> Vec<SavingsAccountItem> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT a.name,
                COALESCE((SELECT balance_cents FROM account_balances b WHERE b.account_id = a.id ORDER BY as_of_date DESC LIMIT 1), 0) AS balance,
                a.apy_pct
         FROM accounts a
         WHERE a.archived_at IS NULL AND a.type = 'Savings'",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return out,
    };
    let rows = match stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<f64>>(2)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return out,
    };
    for row in rows.flatten() {
        let (name, balance_cents, apy_pct) = row;
        out.push(SavingsAccountItem {
            name,
            balance_cents,
            apy_pct,
        });
    }
    out
}

fn loan_context(conn: &mut Connection) -> Vec<LoanDetailItem> {
    let mut out = Vec::new();
    let mut stmt = match conn.prepare(
        "SELECT name, balance_cents, original_balance_cents, apr_pct, started_at
         FROM liabilities
         WHERE liability_type IN ('loan', 'mortgage')
         ORDER BY balance_cents DESC",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return out,
    };
    let rows = match stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, Option<i64>>(2)?,
            r.get::<_, Option<f64>>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    }) {
        Ok(rows) => rows,
        Err(_) => return out,
    };
    for row in rows.flatten() {
        let (name, balance_cents, original_balance_cents, apr_pct, started_at) = row;
        let paid_down_pct = original_balance_cents
            .filter(|o| *o > 0)
            .map(|o| ((o - balance_cents).max(0) * 100 / o).clamp(0, 100));
        out.push(LoanDetailItem {
            name,
            balance_cents,
            original_balance_cents,
            apr_pct,
            started_at,
            paid_down_pct,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{
        db::run_migrations,
        keychain,
        models::{AccountType, NewAccount, NewTransaction, TransactionStatus},
        repos::{accounts, transactions},
        Db,
    };
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("context.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_account(conn: &mut Connection, opening_balance_cents: i64) -> String {
        accounts::insert(
            conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Checking,
                name: "Checking".into(),
                last4: None,
                currency: "USD".into(),
                color: "#123456".into(),
                opening_balance_cents,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: None,
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
            },
        )
        .unwrap()
        .id
    }

    #[test]
    fn test_build_context_on_empty_db() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        let ctx = build_context(&mut conn);

        assert_eq!(ctx.cashflow.total_balance_cents, 0);
        assert_eq!(ctx.cashflow.avg_monthly_income_cents, 0);
        assert_eq!(ctx.cashflow.avg_monthly_expense_cents, 0);
        assert_eq!(ctx.cashflow.net_monthly_cents, 0);
        assert_eq!(ctx.cashflow.savings_rate_pct, 0);
        assert_eq!(ctx.budget.total_budget_cents, 0);
        assert_eq!(ctx.budget.total_spent_cents, 0);
        assert_eq!(ctx.transactions.total_count, 0);
        assert_eq!(ctx.transactions.uncategorized_count, 0);
        assert!(ctx.cashflow.upcoming_bills.is_empty());
        assert!(ctx.budget.overages.is_empty());
        assert!(ctx.budget.near_limit.is_empty());
        assert!(ctx.goals.is_empty());
        assert!(ctx.transactions.top_merchants_this_month.is_empty());
        assert!(ctx.memory.is_empty());
    }

    #[test]
    fn test_wellness_includes_savings_apys_and_loan_history() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();

        accounts::insert(
            &mut conn,
            NewAccount {
                owner: "Me".into(),
                bank: "Bank".into(),
                r#type: AccountType::Savings,
                name: "HISA".into(),
                last4: None,
                currency: "USD".into(),
                color: "#123456".into(),
                opening_balance_cents: 50_000_00,
                source: "manual".into(),
                liquidity_type: "liquid".into(),
                emergency_fund_eligible: true,
                goal_earmark: None,
                apy_pct: Some(4.5),
                simplefin_account_id: None,
                nickname: None,
                connection_id: None,
                institution_id: None,
                external_account_id: None,
                official_name: None,
                mask: None,
                subtype: None,
                account_group: "cash".into(),
                available_balance_cents: None,
                balance_date: None,
                extra_json: None,
                raw_json: None,
                import_pending: false,
            },
        )
        .unwrap();

        use finsight_core::models::NewLiability;
        use finsight_core::repos::liabilities;
        liabilities::create(
            &mut conn,
            NewLiability {
                name: "Car Loan".into(),
                liability_type: "loan".into(),
                balance_cents: 12_000_00,
                limit_cents: None,
                apr_pct: Some(5.9),
                min_payment_cents: None,
                payoff_date: None,
                original_balance_cents: Some(20_000_00),
                started_at: Some("2021-06".into()),
                currency: "USD".into(),
            },
        )
        .unwrap();

        let ctx = build_context(&mut conn);

        assert_eq!(ctx.wellness.savings_accounts.len(), 1);
        assert_eq!(ctx.wellness.savings_accounts[0].apy_pct, Some(4.5));
        assert_eq!(ctx.wellness.loans.len(), 1);
        assert_eq!(
            ctx.wellness.loans[0].original_balance_cents,
            Some(20_000_00)
        );
        assert_eq!(ctx.wellness.loans[0].paid_down_pct, Some(40));
        assert!(ctx.to_prompt_string().contains("4.5% APY"));
        assert!(ctx.to_prompt_string().contains("40% paid down"));
    }

    #[test]
    fn test_build_context_has_sane_cashflow() {
        let (_dir, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let account_id = seed_account(&mut conn, 1_000_000);

        let now = Utc::now();
        for (days_ago, amount, merchant) in [
            (10, 300_000, "Payroll"),
            (40, 300_000, "Payroll"),
            (70, 300_000, "Payroll"),
            (5, -100_000, "Rent"),
            (35, -100_000, "Rent"),
            (65, -100_000, "Rent"),
        ] {
            transactions::insert(
                &mut conn,
                NewTransaction {
                    account_id: account_id.clone(),
                    posted_at: now - Duration::days(days_ago),
                    amount_cents: amount,
                    merchant_raw: merchant.into(),
                    category_id: None,
                    notes: None,
                    status: TransactionStatus::Cleared,
                    imported_id: None,
                    source: None,
                    raw_synced_data: None,
                    pending: false,
                    external_tx_id: None,
                    external_account_id: None,
                },
            )
            .unwrap();
        }

        let ctx = build_context(&mut conn);

        assert_eq!(ctx.cashflow.total_balance_cents, 1_000_000);
        assert_eq!(ctx.cashflow.avg_monthly_income_cents, 300_000);
        assert_eq!(ctx.cashflow.avg_monthly_expense_cents, 100_000);
        assert_eq!(ctx.cashflow.net_monthly_cents, 200_000);
        assert!(ctx.cashflow.savings_rate_pct >= 66);
        assert!(ctx.cashflow.runway_days > 0);
        assert_eq!(ctx.transactions.total_count, 6);
    }
}
