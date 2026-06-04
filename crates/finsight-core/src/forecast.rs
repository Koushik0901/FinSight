//! Pure deterministic projection engine for what-if scenarios.
//! No DB, no LLM — given a financial snapshot and parameters, project trajectories.

/// Runway is capped here so "covered indefinitely" doesn't produce absurd numbers.
pub const RUNWAY_CAP_DAYS: i64 = 3650;

/// Parameters describing a scenario. Built directly from preset chips, or
/// extracted from free text by the LLM in the app layer.
#[derive(Debug, Clone, Default)]
pub struct ScenarioParams {
    /// e.g. -50 means "cut income by 50%".
    pub income_delta_pct: i32,
    /// Recurring monthly outflow change. Positive = more outflow (e.g. add to
    /// savings); negative = less outflow (e.g. eliminate dining).
    pub monthly_expense_delta_cents: i64,
    /// One-off cost in cents, applied at `start_month_offset`.
    pub one_time_cents: i64,
    /// Months from now the change begins (0 = immediately).
    pub start_month_offset: u32,
    /// Human label echoed back for display.
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct GoalInfo {
    pub name: String,
    pub remaining_cents: i64,
    pub monthly_cents: i64,
}

/// Current financial state the projection runs against.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub balance_cents: i64,
    pub avg_monthly_income_cents: i64,
    pub avg_monthly_expense_cents: i64,
    pub goals: Vec<GoalInfo>,
}

#[derive(Debug, Clone)]
pub struct Projection {
    pub baseline_monthly: Vec<i64>,
    pub scenario_monthly: Vec<i64>,
    pub runway_change_days: i64,
    pub monthly_impact_cents: i64,
    pub verdict: bool,
    pub goals_affected: Vec<String>,
    pub considerations: Vec<String>,
}

/// Days a balance lasts given an outflow over a period. Shared formula:
/// `avg_daily = outflow / period_days`, `runway = balance / avg_daily`.
/// TODO §3d's Today runway stat calls this with (balance, expenses_this_month, day_of_month).
pub fn runway_days(balance_cents: i64, period_outflow_cents: i64, period_days: i64) -> i64 {
    if period_outflow_cents <= 0 || period_days <= 0 {
        return RUNWAY_CAP_DAYS;
    }
    let daily = period_outflow_cents as f64 / period_days as f64;
    let days = (balance_cents as f64 / daily).floor() as i64;
    days.clamp(0, RUNWAY_CAP_DAYS)
}

fn fmt_money(cents: i64) -> String {
    format!("${:.0}", (cents.abs() as f64) / 100.0)
}

fn fmt_runway(days: i64) -> String {
    if days >= RUNWAY_CAP_DAYS {
        "10+ years".to_string()
    } else if days >= 365 {
        format!("{:.1} years", days as f64 / 365.0)
    } else {
        format!("{} days", days)
    }
}

pub fn project(s: &Snapshot, p: &ScenarioParams, months: u32) -> Projection {
    let n = months.max(1) as usize;
    let start = (p.start_month_offset as usize).min(n.saturating_sub(1));

    let base_income = s.avg_monthly_income_cents;
    let base_expense = s.avg_monthly_expense_cents;
    let base_net = base_income - base_expense;

    let scen_income =
        (base_income as f64 * (1.0 + p.income_delta_pct as f64 / 100.0)).round() as i64;
    let scen_expense = base_expense + p.monthly_expense_delta_cents;
    let scen_net = scen_income - scen_expense;

    let mut baseline_monthly = Vec::with_capacity(n);
    let mut scenario_monthly = Vec::with_capacity(n);
    let mut bal = s.balance_cents;
    let mut sbal = s.balance_cents;
    for i in 0..n {
        bal += base_net;
        let month_net = if i >= start { scen_net } else { base_net };
        sbal += month_net;
        if i == start {
            sbal -= p.one_time_cents;
        }
        baseline_monthly.push(bal);
        scenario_monthly.push(sbal);
    }

    // Runway uses NET outflow (expense - income), so income cuts shorten it.
    let base_outflow = (base_expense - base_income).max(0);
    let scen_outflow = (scen_expense - scen_income).max(0);
    let base_runway = runway_days(s.balance_cents, base_outflow, 30);
    let scen_runway = runway_days(s.balance_cents - p.one_time_cents, scen_outflow, 30);
    let runway_change_days = scen_runway - base_runway;

    let monthly_impact_cents = scen_net - base_net;

    let verdict = scenario_monthly.iter().all(|&v| v >= 0);

    // Goals affected: distribute the monthly shortfall proportionally across goals.
    let total_goal_monthly: i64 = s.goals.iter().map(|g| g.monthly_cents.max(0)).sum();
    let shortfall = (base_net - scen_net).max(0);
    let mut goals_affected = Vec::new();
    if shortfall > 0 && total_goal_monthly > 0 {
        for g in &s.goals {
            if g.monthly_cents <= 0 || g.remaining_cents <= 0 {
                continue;
            }
            let share = (shortfall as f64 * (g.monthly_cents as f64 / total_goal_monthly as f64))
                .round() as i64;
            let new_monthly = g.monthly_cents - share;
            let base_eta = (g.remaining_cents + g.monthly_cents - 1) / g.monthly_cents;
            if new_monthly <= 0 {
                goals_affected.push(format!("{}: paused", g.name));
            } else {
                let scen_eta = (g.remaining_cents + new_monthly - 1) / new_monthly;
                let slip = scen_eta - base_eta;
                if slip > 0 {
                    goals_affected.push(format!("{}: +{} mo", g.name, slip));
                }
            }
        }
    }

    let considerations = build_considerations(
        s,
        n,
        base_runway,
        scen_runway,
        runway_change_days,
        monthly_impact_cents,
        &scenario_monthly,
        &goals_affected,
        verdict,
    );

    Projection {
        baseline_monthly,
        scenario_monthly,
        runway_change_days,
        monthly_impact_cents,
        verdict,
        goals_affected,
        considerations,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_considerations(
    s: &Snapshot,
    n: usize,
    base_runway: i64,
    scen_runway: i64,
    runway_change: i64,
    monthly_impact: i64,
    scenario_monthly: &[i64],
    goals_affected: &[String],
    verdict: bool,
) -> Vec<String> {
    let mut out = Vec::new();

    if runway_change < -1 {
        out.push(format!(
            "Runway shortens by {} days — from {} to {}.",
            runway_change.abs(),
            fmt_runway(base_runway),
            fmt_runway(scen_runway)
        ));
    } else if runway_change > 1 {
        out.push(format!(
            "Runway extends by {} days — from {} to {}.",
            runway_change,
            fmt_runway(base_runway),
            fmt_runway(scen_runway)
        ));
    } else {
        out.push("Runway is essentially unchanged.".to_string());
    }

    if s.avg_monthly_expense_cents > 0 {
        let today_months = s.balance_cents as f64 / s.avg_monthly_expense_cents as f64;
        let low = *scenario_monthly.iter().min().unwrap_or(&s.balance_cents);
        let low_months = low as f64 / s.avg_monthly_expense_cents as f64;
        out.push(format!(
            "Your savings cover ~{:.1} months of expenses today; this scenario draws that to ~{:.1} months at its lowest.",
            today_months.max(0.0),
            low_months.max(0.0)
        ));
    }

    if monthly_impact < 0 {
        out.push(format!(
            "This costs about {} more per month than your current plan.",
            fmt_money(monthly_impact)
        ));
    } else if monthly_impact > 0 {
        out.push(format!(
            "This frees about {} per month versus your current plan.",
            fmt_money(monthly_impact)
        ));
    }

    if !goals_affected.is_empty() {
        out.push(format!(
            "Affects {} goal(s): {}.",
            goals_affected.len(),
            goals_affected.join(", ")
        ));
    }

    if verdict {
        out.push(format!(
            "Your projected balance stays positive across the {}-month horizon.",
            n
        ));
    } else {
        let k = scenario_monthly
            .iter()
            .position(|&v| v < 0)
            .map(|i| i + 1)
            .unwrap_or(n);
        out.push(format!(
            "Your projected balance would go negative around month {} — you'd need to adjust spending or income.",
            k
        ));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap() -> Snapshot {
        Snapshot {
            balance_cents: 2_000_000, // $20k
            avg_monthly_income_cents: 600_000, // $6k
            avg_monthly_expense_cents: 400_000, // $4k
            goals: vec![GoalInfo {
                name: "House Fund".into(),
                remaining_cents: 1_200_000,
                monthly_cents: 100_000,
            }],
        }
    }

    #[test]
    fn runway_zero_burn_is_capped() {
        assert_eq!(runway_days(100_000, 0, 30), RUNWAY_CAP_DAYS);
    }

    #[test]
    fn runway_basic_division() {
        // $3000 balance, $3000/mo outflow over 30 days => 30 days.
        assert_eq!(runway_days(300_000, 300_000, 30), 30);
    }

    #[test]
    fn neutral_scenario_is_coverable() {
        let p = ScenarioParams::default();
        let proj = project(&snap(), &p, 12);
        assert_eq!(proj.baseline_monthly.len(), 12);
        assert_eq!(proj.scenario_monthly.len(), 12);
        assert!(proj.verdict);
        // Neutral params => trajectories identical.
        assert_eq!(proj.baseline_monthly, proj.scenario_monthly);
    }

    #[test]
    fn income_cut_shortens_runway() {
        let p = ScenarioParams { income_delta_pct: -100, ..Default::default() };
        let proj = project(&snap(), &p, 12);
        // With no income, net outflow becomes positive => finite, shorter runway.
        assert!(proj.runway_change_days < 0);
    }

    #[test]
    fn one_time_purchase_reduces_scenario_balance() {
        let p = ScenarioParams { one_time_cents: 500_000, ..Default::default() };
        let proj = project(&snap(), &p, 12);
        // First month scenario balance is $5k below baseline.
        assert_eq!(proj.baseline_monthly[0] - proj.scenario_monthly[0], 500_000);
    }

    #[test]
    fn large_one_time_on_low_balance_is_not_coverable() {
        let mut s = snap();
        s.balance_cents = 100_000; // $1k
        let p = ScenarioParams { one_time_cents: 3_500_000, ..Default::default() };
        let proj = project(&s, &p, 12);
        assert!(!proj.verdict);
        assert!(!proj.considerations.is_empty());
    }
}
