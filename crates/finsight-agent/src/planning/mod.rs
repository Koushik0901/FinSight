use crate::finance::{
    self, CashInflowAdvice, CashflowTimeline, DataQualityReport, DebtGoalComparison,
    DebtPayoffRanking, DebtPayoffScenarios, EmergencyFundScenarios, FinanceQuestionKind,
    FinancialSnapshot, GoalAllocationScenarios, GoalConflictScenario, GoalEtaResult,
    PurchaseAffordabilityScenario,
};
use chrono::{NaiveDate, Utc};
use finsight_core::models::MissingDataItem;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinanceTaskType {
    CashInflowAllocation,
    GoalEta,
    DebtRanking,
    DebtPayoffScenario,
    DebtVsGoal,
    GoalAllocation,
    GoalConflict,
    EmergencyFundPlanning,
    CashflowTimeline,
    PurchaseAffordability,
    DataQualityReport,
    FinancialSnapshot,
    InvestmentReadiness,
    GeneralFinancePlanning,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancePlan {
    pub task_type: FinanceTaskType,
    pub required_tools: Vec<String>,
    pub optional_tools: Vec<String>,
    pub required_inputs: Vec<String>,
    pub missing_inputs: Vec<String>,
    pub planning_notes: Vec<String>,
    pub risk_flags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEvidence {
    pub tool_name: String,
    pub summary: String,
    pub data_sources: Vec<String>,
    pub missing_data: Vec<MissingDataItem>,
    pub numbers_used: Vec<NumberUsed>,
    pub raw_json: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberUsed {
    pub label: String,
    pub value: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinanceAlternative {
    pub name: String,
    pub summary: String,
    pub tradeoff: String,
    pub numbers_used: Vec<NumberUsed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredFinanceAnswer {
    pub recommendation: String,
    pub summary: String,
    pub alternatives: Vec<FinanceAlternative>,
    pub numbers_used: Vec<NumberUsed>,
    pub data_sources: Vec<String>,
    pub assumptions: Vec<String>,
    pub missing_data: Vec<MissingDataItem>,
    pub risks: Vec<String>,
    pub next_actions: Vec<String>,
    pub what_would_change_recommendation: Vec<String>,
    pub confidence: f64,
    pub reasoning: String,
    pub trace: Vec<String>,
    pub follow_up_questions: Vec<String>,
    pub verification: VerificationReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerificationSeverity {
    Ok,
    Warning,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub passed: bool,
    pub severity: VerificationSeverity,
    pub findings: Vec<String>,
    pub confidence_adjustment: f64,
    pub required_follow_up_questions: Vec<String>,
}

enum EvidencePayload {
    Snapshot(FinancialSnapshot),
    CashInflow(CashInflowAdvice),
    GoalEta(GoalEtaResult),
    DebtRanking(DebtPayoffRanking),
    DebtPayoffScenario(DebtPayoffScenarios),
    DebtVsGoal(DebtGoalComparison),
    GoalAllocation(GoalAllocationScenarios),
    GoalConflict(GoalConflictScenario),
    EmergencyFund(EmergencyFundScenarios),
    CashflowTimeline(CashflowTimeline),
    PurchaseAffordability(PurchaseAffordabilityScenario),
    DataQuality(DataQualityReport),
}

struct EvidenceRecord {
    evidence: ToolEvidence,
    payload: EvidencePayload,
}

pub fn answer_finance_question(
    conn: &mut Connection,
    question: &str,
) -> anyhow::Result<Option<StructuredFinanceAnswer>> {
    let plan = plan_finance_question(question);
    if plan.task_type == FinanceTaskType::Unknown {
        return Ok(None);
    }
    if !plan.missing_inputs.is_empty() {
        let mut answer = blocked_for_missing_inputs(&plan);
        answer.verification = verify_answer(&plan, &[], &answer);
        return Ok(Some(answer));
    }

    let evidence = execute_plan(conn, question, &plan)?;
    let mut answer = build_answer(question, &plan, &evidence)?;
    answer.verification = verify_answer(&plan, &evidence, &answer);
    apply_verification(&mut answer);
    Ok(Some(answer))
}

pub fn plan_finance_question(question: &str) -> FinancePlan {
    let profile = finance::infer_question_profile(question);
    let mut plan = FinancePlan {
        task_type: map_task_type(profile.kind, question),
        required_tools: vec!["get_financial_snapshot".to_string()],
        optional_tools: Vec::new(),
        required_inputs: Vec::new(),
        missing_inputs: Vec::new(),
        planning_notes: Vec::new(),
        risk_flags: Vec::new(),
    };

    match plan.task_type {
        FinanceTaskType::CashInflowAllocation => {
            plan.required_tools.push("analyze_cash_inflow".to_string());
            plan.required_inputs.push("amount_cents".to_string());
            if profile.amount_cents.unwrap_or(0) <= 0 {
                plan.missing_inputs
                    .push("How much is the paycheck or windfall, in dollars?".to_string());
            }
        }
        FinanceTaskType::GoalEta => {
            plan.required_tools.push("calculate_goal_eta".to_string());
            plan.required_inputs
                .extend(["goal_id".to_string(), "contribution_cents".to_string()]);
            if profile.amount_cents.unwrap_or(0) <= 0 {
                plan.missing_inputs
                    .push("How much do you want to save each pay period?".to_string());
            }
        }
        FinanceTaskType::DebtRanking => {
            plan.required_tools.push("rank_debt_payoff".to_string());
        }
        FinanceTaskType::DebtPayoffScenario => {
            plan.required_tools
                .push("run_debt_payoff_scenarios".to_string());
        }
        FinanceTaskType::DebtVsGoal => {
            plan.required_tools.push("compare_debt_vs_goal".to_string());
            plan.required_inputs.push("goal_id".to_string());
        }
        FinanceTaskType::GoalAllocation => {
            plan.required_tools
                .push("run_goal_allocation_scenarios".to_string());
            plan.required_inputs
                .push("monthly_available_cents".to_string());
            if profile.amount_cents.unwrap_or(0) <= 0 {
                plan.missing_inputs
                    .push("How much monthly savings should I allocate across goals?".to_string());
            }
        }
        FinanceTaskType::GoalConflict => {
            plan.required_tools
                .push("run_goal_conflict_scenario".to_string());
            plan.required_inputs
                .extend(["goal_id".to_string(), "contribution_cents".to_string()]);
            if profile.amount_cents.unwrap_or(0) <= 0 {
                plan.missing_inputs
                    .push("How much do you want to contribute to the goal?".to_string());
            }
        }
        FinanceTaskType::EmergencyFundPlanning => {
            plan.required_tools
                .push("run_emergency_fund_scenarios".to_string());
        }
        FinanceTaskType::CashflowTimeline => {
            plan.required_tools
                .push("run_cashflow_timeline".to_string());
        }
        FinanceTaskType::PurchaseAffordability => {
            plan.required_tools
                .push("run_purchase_affordability".to_string());
            plan.required_inputs
                .push("purchase_amount_cents".to_string());
            if profile.amount_cents.unwrap_or(0) <= 0 {
                plan.missing_inputs
                    .push("What is the purchase amount, in dollars?".to_string());
            }
        }
        FinanceTaskType::DataQualityReport => {
            plan.required_tools
                .push("get_data_quality_report".to_string());
        }
        FinanceTaskType::FinancialSnapshot => {}
        FinanceTaskType::InvestmentReadiness => {
            plan.risk_flags
                .push("investment_principles_only".to_string());
        }
        FinanceTaskType::GeneralFinancePlanning => {
            plan.optional_tools.extend([
                "get_budgets".to_string(),
                "run_cashflow_projection".to_string(),
                "run_debt_payoff_scenarios".to_string(),
                "run_goal_allocation_scenarios".to_string(),
                "run_goal_conflict_scenario".to_string(),
                "run_emergency_fund_scenarios".to_string(),
                "run_cashflow_timeline".to_string(),
                "get_data_quality_report".to_string(),
            ]);
        }
        FinanceTaskType::Unknown => {}
    }

    plan.required_tools.sort();
    plan.required_tools.dedup();
    plan
}

fn map_task_type(kind: FinanceQuestionKind, question: &str) -> FinanceTaskType {
    let lower = question.to_lowercase();
    if mentions_investing(question) {
        return FinanceTaskType::InvestmentReadiness;
    }
    if contains_any(
        &lower,
        &[
            "data quality",
            "missing data",
            "what data",
            "what information",
            "is my data complete",
        ],
    ) {
        return FinanceTaskType::DataQualityReport;
    }
    if contains_any(
        &lower,
        &[
            "emergency fund",
            "runway",
            "income loss",
            "lose my job",
            "lost my job",
        ],
    ) {
        return FinanceTaskType::EmergencyFundPlanning;
    }
    if contains_any(
        &lower,
        &["afford", "can i buy", "should i buy", "large purchase"],
    ) {
        return FinanceTaskType::PurchaseAffordability;
    }
    if contains_any(
        &lower,
        &[
            "cashflow timeline",
            "cash flow timeline",
            "end of month",
            "monthly balance",
            "low balance",
        ],
    ) {
        return FinanceTaskType::CashflowTimeline;
    }
    if contains_any(
        &lower,
        &[
            "goal conflict",
            "upcoming bills",
            "upcoming bill",
            "recurring bills",
            "recurring bill",
            "fund my goal",
            "contribute to my goal",
            "save to my goal",
        ],
    ) {
        return FinanceTaskType::GoalConflict;
    }
    if contains_any(
        &lower,
        &[
            "allocate across goals",
            "split across goals",
            "which goals",
            "multiple goals",
            "goal allocation",
        ],
    ) {
        return FinanceTaskType::GoalAllocation;
    }
    if contains_any(
        &lower,
        &[
            "payoff timeline",
            "payoff scenarios",
            "months to pay off",
            "how long to pay off",
            "extra payment",
            "extra monthly",
            "interest saved",
        ],
    ) {
        return FinanceTaskType::DebtPayoffScenario;
    }
    match kind {
        FinanceQuestionKind::CashInflow => FinanceTaskType::CashInflowAllocation,
        FinanceQuestionKind::GoalEta => FinanceTaskType::GoalEta,
        FinanceQuestionKind::DebtVsGoal => FinanceTaskType::DebtVsGoal,
        FinanceQuestionKind::DebtRanking => FinanceTaskType::DebtRanking,
        FinanceQuestionKind::Snapshot => FinanceTaskType::FinancialSnapshot,
        FinanceQuestionKind::GeneralPlanning => FinanceTaskType::GeneralFinancePlanning,
        FinanceQuestionKind::Unknown => FinanceTaskType::Unknown,
    }
}

fn execute_plan(
    conn: &mut Connection,
    question: &str,
    plan: &FinancePlan,
) -> anyhow::Result<Vec<EvidenceRecord>> {
    let snapshot = finance::build_snapshot(conn)?;
    let mut out = vec![EvidenceRecord {
        evidence: snapshot_evidence(&snapshot)?,
        payload: EvidencePayload::Snapshot(snapshot.clone()),
    }];

    match plan.task_type {
        FinanceTaskType::CashInflowAllocation => {
            let amount = finance::parse_amount_cents(question).unwrap_or_default();
            let advice = finance::analyze_cash_inflow(conn, amount)?;
            out.push(EvidenceRecord {
                evidence: cash_inflow_evidence(&advice)?,
                payload: EvidencePayload::CashInflow(advice),
            });
        }
        FinanceTaskType::GoalEta => {
            let amount = finance::parse_amount_cents(question).unwrap_or_default();
            let cadence = finance::infer_cadence(question).unwrap_or("monthly");
            let Some(goal) = find_best_goal_match(question, &snapshot.goals) else {
                return Ok(vec![out.remove(0)]);
            };
            let eta = finance::calculate_goal_eta(conn, &goal.id, amount, cadence)?;
            out.push(EvidenceRecord {
                evidence: goal_eta_evidence(&eta)?,
                payload: EvidencePayload::GoalEta(eta),
            });
        }
        FinanceTaskType::DebtRanking => {
            let method = if question.to_lowercase().contains("snowball") {
                "snowball"
            } else {
                "avalanche"
            };
            let ranking = finance::rank_debt_payoff(conn, method)?;
            out.push(EvidenceRecord {
                evidence: debt_ranking_evidence(&ranking)?,
                payload: EvidencePayload::DebtRanking(ranking),
            });
        }
        FinanceTaskType::DebtPayoffScenario => {
            let method = if question.to_lowercase().contains("snowball") {
                "snowball"
            } else {
                "avalanche"
            };
            let extra = finance::parse_amount_cents(question).unwrap_or(0);
            let scenarios = finance::run_debt_payoff_scenarios(conn, method, extra)?;
            out.push(EvidenceRecord {
                evidence: debt_payoff_scenario_evidence(&scenarios)?,
                payload: EvidencePayload::DebtPayoffScenario(scenarios),
            });
        }
        FinanceTaskType::DebtVsGoal => {
            let Some(goal) = find_best_goal_match(question, &snapshot.goals) else {
                return Ok(vec![out.remove(0)]);
            };
            let liability = find_best_liability_match(question, &snapshot.liabilities);
            let comparison =
                finance::compare_debt_vs_goal(conn, &goal.id, liability.map(|d| d.id.as_str()))?;
            out.push(EvidenceRecord {
                evidence: debt_vs_goal_evidence(&comparison)?,
                payload: EvidencePayload::DebtVsGoal(comparison),
            });
        }
        FinanceTaskType::GoalAllocation => {
            let amount = finance::parse_amount_cents(question).unwrap_or_default();
            let strategy = if question.to_lowercase().contains("deadline") {
                "deadline"
            } else if question.to_lowercase().contains("proportional") {
                "proportional"
            } else {
                "priority"
            };
            let scenarios = finance::run_goal_allocation_scenarios(conn, amount, strategy)?;
            out.push(EvidenceRecord {
                evidence: goal_allocation_evidence(&scenarios)?,
                payload: EvidencePayload::GoalAllocation(scenarios),
            });
        }
        FinanceTaskType::GoalConflict => {
            let amount = finance::parse_amount_cents(question).unwrap_or_default();
            let Some(goal) = find_best_goal_match(question, &snapshot.goals) else {
                return Ok(vec![out.remove(0)]);
            };
            let scenario = finance::run_goal_conflict_scenario(conn, &goal.id, amount)?;
            out.push(EvidenceRecord {
                evidence: goal_conflict_evidence(&scenario)?,
                payload: EvidencePayload::GoalConflict(scenario),
            });
        }
        FinanceTaskType::EmergencyFundPlanning => {
            let contribution = finance::parse_amount_cents(question).unwrap_or(0);
            let scenarios = finance::run_emergency_fund_scenarios(conn, contribution)?;
            out.push(EvidenceRecord {
                evidence: emergency_fund_evidence(&scenarios)?,
                payload: EvidencePayload::EmergencyFund(scenarios),
            });
        }
        FinanceTaskType::CashflowTimeline => {
            let timeline = finance::run_cashflow_timeline(conn, 3)?;
            out.push(EvidenceRecord {
                evidence: cashflow_timeline_evidence(&timeline)?,
                payload: EvidencePayload::CashflowTimeline(timeline),
            });
        }
        FinanceTaskType::PurchaseAffordability => {
            let amount = finance::parse_amount_cents(question).unwrap_or_default();
            let scenario = finance::run_purchase_affordability(conn, amount)?;
            out.push(EvidenceRecord {
                evidence: purchase_affordability_evidence(&scenario)?,
                payload: EvidencePayload::PurchaseAffordability(scenario),
            });
        }
        FinanceTaskType::DataQualityReport => {
            let report = finance::get_data_quality_report(conn)?;
            out.push(EvidenceRecord {
                evidence: data_quality_evidence(&report)?,
                payload: EvidencePayload::DataQuality(report),
            });
        }
        FinanceTaskType::FinancialSnapshot
        | FinanceTaskType::InvestmentReadiness
        | FinanceTaskType::GeneralFinancePlanning
        | FinanceTaskType::Unknown => {}
    }

    Ok(out)
}

fn build_answer(
    question: &str,
    plan: &FinancePlan,
    evidence: &[EvidenceRecord],
) -> anyhow::Result<StructuredFinanceAnswer> {
    let snapshot = evidence.iter().find_map(|item| match &item.payload {
        EvidencePayload::Snapshot(value) => Some(value),
        _ => None,
    });
    let mut answer = base_answer(plan, evidence);

    match plan.task_type {
        FinanceTaskType::CashInflowAllocation => {
            let Some(advice) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::CashInflow(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need the cash inflow amount before I can allocate it.".to_string();
                answer
                    .follow_up_questions
                    .push("How much is the paycheck or windfall?".to_string());
                return Ok(answer);
            };
            answer.recommendation = format!(
                "For {}, prioritize emergency cash, high-interest debt, then goals.",
                format_cents(advice.amount_cents)
            );
            answer.summary = advice
                .allocations
                .iter()
                .map(|allocation| {
                    format!(
                        "{}: {} ({})",
                        allocation.bucket.replace('_', " "),
                        format_cents(allocation.amount_cents),
                        allocation.reason
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            answer.reasoning = advice.rationale.join(" ");
            answer.missing_data.extend(advice.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.assumptions.push(if advice.investing_allowed {
                "Investing readiness passed the emergency fund and high-interest debt checks."
                    .to_string()
            } else {
                "Investing is deferred until emergency coverage and debt priorities are addressed."
                    .to_string()
            });
            answer
                .next_actions
                .push("Review the proposed allocation before moving money.".to_string());
        }
        FinanceTaskType::GoalEta => {
            let Some(eta) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::GoalEta(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need the specific goal before I can estimate the timeline.".to_string();
                answer
                    .follow_up_questions
                    .push("Which goal should I use for the ETA?".to_string());
                return Ok(answer);
            };
            let eta_text = eta
                .eta_months
                .map(|m| format!("{m} month(s)"))
                .unwrap_or_else(|| "an unknown timeline".to_string());
            answer.recommendation = format!(
                "At {} {}, you should reach {} in about {}.",
                format_cents(eta.contribution_cents),
                eta.cadence,
                eta.goal_name,
                eta_text
            );
            answer.summary = format!(
                "That contribution equals about {} per month against {} remaining.",
                format_cents(eta.monthly_equivalent_cents),
                format_cents(eta.remaining_cents)
            );
            answer.reasoning = answer.summary.clone();
            answer
                .next_actions
                .push("Update the goal contribution if this timeline looks right.".to_string());
        }
        FinanceTaskType::DebtRanking => {
            let Some(ranking) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::DebtRanking(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation = "I do not see active debts to rank.".to_string();
                return Ok(answer);
            };
            let ordered = ranking
                .items
                .iter()
                .map(|item| {
                    format!(
                        "{}. {} ({}, {})",
                        item.rank,
                        item.name,
                        format_cents(item.balance_cents),
                        item.reason
                    )
                })
                .collect::<Vec<_>>();
            answer.recommendation = if ordered.is_empty() {
                "I do not see any active debts to rank.".to_string()
            } else {
                format!("Use {} ordering.", ranking.method)
            };
            answer.summary = ordered.join(" ");
            answer.reasoning = format!("{} debts ranked with {}.", ordered.len(), ranking.method);
            answer.missing_data.extend(ranking.missing_data.iter().cloned().map(MissingDataItem::from));
            answer
                .next_actions
                .push("Use this order for extra payments after minimums are covered.".to_string());
        }
        FinanceTaskType::DebtPayoffScenario => {
            let Some(scenarios) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::DebtPayoffScenario(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation = "I need complete APR and minimum payment data before modeling payoff timelines.".to_string();
                return Ok(answer);
            };
            answer.recommendation = format!(
                "Use {} payoff ordering; apply extra debt dollars to the next priority debt after minimums.",
                scenarios.method
            );
            answer.summary = match (
                scenarios.payoff_months_minimums_only,
                scenarios.payoff_months_with_extra,
                scenarios.estimated_interest_saved_cents,
            ) {
                (Some(base), Some(extra), Some(saved)) => format!(
                    "Minimums-only payoff is about {base} month(s); with {} extra per month, payoff is about {extra} month(s), saving about {} of interest.",
                    format_cents(scenarios.extra_monthly_payment_cents),
                    format_cents(saved)
                ),
                _ => "Payoff timing is provisional because APR or minimum-payment data is missing.".to_string(),
            };
            answer.reasoning = scenarios.assumptions.join(" ");
            answer.missing_data.extend(scenarios.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.alternatives = vec![
                FinanceAlternative {
                    name: "Minimum payments only".to_string(),
                    summary: scenarios
                        .payoff_months_minimums_only
                        .map(|m| format!("Debt-free in about {m} month(s)."))
                        .unwrap_or_else(|| "Timeline unknown.".to_string()),
                    tradeoff: "Preserves monthly cashflow but usually costs the most interest."
                        .to_string(),
                    numbers_used: Vec::new(),
                },
                FinanceAlternative {
                    name: "Minimums plus extra payment".to_string(),
                    summary: scenarios
                        .payoff_months_with_extra
                        .map(|m| format!("Debt-free in about {m} month(s)."))
                        .unwrap_or_else(|| "Timeline unknown.".to_string()),
                    tradeoff: "Uses more monthly cash but shortens payoff and interest."
                        .to_string(),
                    numbers_used: Vec::new(),
                },
            ];
            answer.next_actions.push(
                "Confirm APR and minimum-payment data, then choose the monthly extra payment amount."
                    .to_string(),
            );
        }
        FinanceTaskType::DebtVsGoal => {
            let Some(comparison) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::DebtVsGoal(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need the goal name before I can compare it against debt.".to_string();
                answer
                    .follow_up_questions
                    .push("Which savings goal should I compare?".to_string());
                return Ok(answer);
            };
            answer.recommendation = comparison.recommendation.clone();
            let mut summary = Vec::new();
            if comparison.suggested_goal_drawdown_cents > 0 {
                summary.push(format!(
                    "Safe drawdown: {}, leaving {:.1} month(s) of emergency coverage.",
                    format_cents(comparison.suggested_goal_drawdown_cents),
                    comparison.emergency_fund_months_after_drawdown
                ));
            }
            if let Some(saved) = comparison.estimated_interest_saved_cents {
                summary.push(format!(
                    "Estimated interest avoided: {}.",
                    format_cents(saved)
                ));
            }
            answer.summary = summary.join(" ");
            answer.reasoning = comparison.rationale.join(" ");
            answer.missing_data.extend(comparison.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.alternatives = comparison
                .alternatives
                .iter()
                .map(|alternative| FinanceAlternative {
                    name: alternative.name.clone(),
                    summary: alternative.action.clone(),
                    tradeoff: alternative.tradeoff.clone(),
                    numbers_used: vec![
                        NumberUsed {
                            label: "cash used".to_string(),
                            value: format_cents(alternative.cash_used_cents),
                            source: "compare_debt_vs_goal".to_string(),
                        },
                        NumberUsed {
                            label: "emergency fund after action".to_string(),
                            value: format!("{:.1} months", alternative.emergency_fund_months),
                            source: "compare_debt_vs_goal".to_string(),
                        },
                    ],
                })
                .collect();
            answer
                .next_actions
                .push("Choose whether preserving car progress or reducing debt faster matters more right now.".to_string());
        }
        FinanceTaskType::GoalAllocation => {
            let Some(scenarios) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::GoalAllocation(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need a monthly savings amount before allocating across goals.".to_string();
                return Ok(answer);
            };
            answer.recommendation = format!(
                "Allocate {} per month across goals using the {} strategy.",
                format_cents(scenarios.monthly_available_cents),
                scenarios.strategy
            );
            answer.summary = scenarios
                .allocations
                .iter()
                .map(|item| {
                    let eta = item
                        .eta_months
                        .map(|m| format!("about {m} month(s)"))
                        .unwrap_or_else(|| "timeline unknown".to_string());
                    format!(
                        "{}: {}/mo, {}.",
                        item.goal_name,
                        format_cents(item.suggested_monthly_cents),
                        eta
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            answer.reasoning = scenarios.assumptions.join(" ");
            answer.missing_data.extend(scenarios.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.next_actions.push(
                "Review whether the modeled goal priority matches your real preferences."
                    .to_string(),
            );
        }
        FinanceTaskType::GoalConflict => {
            let Some(scenario) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::GoalConflict(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need a goal and contribution amount before comparing upcoming bills."
                        .to_string();
                answer
                    .follow_up_questions
                    .push("Which goal and how much do you want to contribute?".to_string());
                return Ok(answer);
            };
            answer.recommendation = scenario.recommendation.clone();
            answer.summary = format!(
                "Goal contribution: {}; upcoming obligations: {}; emergency floor: {}; safe contribution now: {}; cash after full contribution and obligations: {}.",
                format_cents(scenario.requested_contribution_cents),
                format_cents(scenario.upcoming_obligations_cents),
                format_cents(scenario.emergency_floor_cents),
                format_cents(scenario.safe_contribution_now_cents),
                format_cents(scenario.emergency_fund_after_full_contribution_cents)
            );
            answer.reasoning = scenario.assumptions.join(" ");
            answer.missing_data.extend(scenario.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.alternatives = scenario
                .alternatives
                .iter()
                .map(|alternative| FinanceAlternative {
                    name: alternative.name.clone(),
                    summary: alternative.action.clone(),
                    tradeoff: alternative.tradeoff.clone(),
                    numbers_used: vec![
                        NumberUsed {
                            label: "goal contribution".to_string(),
                            value: format_cents(alternative.goal_contribution_cents),
                            source: "run_goal_conflict_scenario".to_string(),
                        },
                        NumberUsed {
                            label: "cash after obligations".to_string(),
                            value: format_cents(alternative.cash_after_obligations_cents),
                            source: "run_goal_conflict_scenario".to_string(),
                        },
                    ],
                })
                .collect();
            if scenario.conflicts_with_cashflow {
                answer.risks.push(
                    "Goal funding conflicts with modeled upcoming bills, monthly surplus, or the emergency floor."
                        .to_string(),
                );
            }
            answer.next_actions.push(
                "Pay or confirm the upcoming bills first, then rerun the goal contribution amount."
                    .to_string(),
            );
        }
        FinanceTaskType::EmergencyFundPlanning => {
            let Some(scenarios) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::EmergencyFund(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need expense and balance data before modeling your emergency fund."
                        .to_string();
                return Ok(answer);
            };
            answer.recommendation = if scenarios.current_months < 1.0 {
                "Prioritize reaching a one-month emergency fund before extra goals or investing."
                    .to_string()
            } else if scenarios.current_months < 3.0 {
                "Build toward a three-month emergency fund while keeping debt and goal tradeoffs visible.".to_string()
            } else {
                "Emergency coverage is in a healthier range; compare debt, goals, and investing readiness next.".to_string()
            };
            answer.summary = format!(
                "Current liquid coverage is {:.1} month(s). One/three/six-month gaps: {}.",
                scenarios.current_months,
                scenarios
                    .targets
                    .iter()
                    .map(|target| format!(
                        "{} mo = {} gap",
                        target.target_months,
                        format_cents(target.gap_cents)
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            answer.reasoning = scenarios.assumptions.join(" ");
            answer.missing_data.extend(scenarios.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.next_actions.push(
                "Choose a monthly emergency-fund contribution and rerun the target timeline."
                    .to_string(),
            );
        }
        FinanceTaskType::CashflowTimeline => {
            let Some(timeline) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::CashflowTimeline(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need cashflow data before building a timeline.".to_string();
                return Ok(answer);
            };
            answer.recommendation = if timeline.low_balance_warnings.is_empty() {
                "The modeled cashflow timeline does not show a low-balance warning in the requested window.".to_string()
            } else {
                "Protect cashflow before adding new debt payments or goal contributions."
                    .to_string()
            };
            answer.summary = timeline
                .months
                .iter()
                .map(|month| {
                    format!(
                        "Month {} ends around {}.",
                        month.month_index,
                        format_cents(month.ending_balance_cents)
                    )
                })
                .collect::<Vec<_>>()
                .join(" ");
            answer.reasoning = timeline.assumptions.join(" ");
            answer.missing_data.extend(timeline.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.risks.extend(timeline.low_balance_warnings.clone());
            answer.next_actions.push(
                "Add exact paycheck cadence and bill due dates for a sharper timeline.".to_string(),
            );
        }
        FinanceTaskType::PurchaseAffordability => {
            let Some(scenario) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::PurchaseAffordability(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need the purchase amount before I can judge affordability.".to_string();
                answer
                    .follow_up_questions
                    .push("What is the purchase amount, in dollars?".to_string());
                return Ok(answer);
            };
            answer.recommendation = scenario.recommendation.clone();
            answer.summary = format!(
                "Purchase: {}; emergency cash after purchase: {} ({:.1} month(s)); monthly surplus: {}; high-interest debt: {}; wait/save timeline: {}.",
                format_cents(scenario.purchase_amount_cents),
                format_cents(scenario.emergency_fund_after_purchase_cents),
                scenario.emergency_months_after_purchase,
                format_cents(scenario.monthly_surplus_cents),
                format_cents(scenario.high_interest_debt_cents),
                scenario
                    .months_to_save_without_touching_emergency_floor
                    .map(|m| format!("{m} month(s)"))
                    .unwrap_or_else(|| "unknown without monthly surplus".to_string())
            );
            answer.reasoning = scenario.assumptions.join(" ");
            answer.missing_data.extend(scenario.missing_data.iter().cloned().map(MissingDataItem::from));
            answer.alternatives = scenario
                .alternatives
                .iter()
                .map(|alternative| FinanceAlternative {
                    name: alternative.name.clone(),
                    summary: alternative.action.clone(),
                    tradeoff: alternative.tradeoff.clone(),
                    numbers_used: vec![
                        NumberUsed {
                            label: "cash used".to_string(),
                            value: format_cents(alternative.cash_used_cents),
                            source: "run_purchase_affordability".to_string(),
                        },
                        NumberUsed {
                            label: "emergency cash after".to_string(),
                            value: format_cents(alternative.emergency_fund_after_cents),
                            source: "run_purchase_affordability".to_string(),
                        },
                    ],
                })
                .collect();
            if !scenario.affordable_now {
                answer.risks.push(
                    "Purchase is not currently affordable under the local-data emergency/debt/cashflow gates."
                        .to_string(),
                );
            }
            answer.next_actions.push(
                "Decide whether to wait/save, reduce the purchase size, or update missing cashflow/debt data before buying."
                    .to_string(),
            );
        }
        FinanceTaskType::DataQualityReport => {
            let Some(report) = evidence.iter().find_map(|item| match &item.payload {
                EvidencePayload::DataQuality(value) => Some(value),
                _ => None,
            }) else {
                answer.recommendation =
                    "I need local data access before checking data quality.".to_string();
                return Ok(answer);
            };
            answer.recommendation = if report.warnings.is_empty() {
                "Your local data has no major planning-data warnings right now.".to_string()
            } else {
                "Fix the missing or incomplete data before relying on high-confidence planning recommendations.".to_string()
            };
            answer.summary = format!(
                "Missing APRs: {}; missing minimum payments: {}; uncategorized expenses: {}.",
                report.missing_apr_count,
                report.missing_min_payment_count,
                report.uncategorized_expense_count
            );
            answer.reasoning = "Data quality report checks liabilities, transactions, goals, and planned transactions used by planning tools.".to_string();
            answer.missing_data.extend(report.warnings.iter().cloned().map(MissingDataItem::from));
            answer.next_actions.push(
                "Fill in APRs, minimum payments, and uncategorized expenses first.".to_string(),
            );
        }
        FinanceTaskType::FinancialSnapshot | FinanceTaskType::GeneralFinancePlanning => {
            if let Some(snapshot) = snapshot {
                answer.recommendation =
                    "Use the current snapshot as the starting point for planning.".to_string();
                answer.summary = format!(
                    "Liquid balance: {}; total balance: {}; emergency coverage: {:.1} month(s).",
                    format_cents(snapshot.liquid_balance_cents),
                    format_cents(snapshot.total_account_balance_cents),
                    snapshot.emergency_fund_months
                );
                answer.reasoning =
                    "Snapshot built from accounts, transactions, goals, debts, bills, and planned transactions."
                        .to_string();
                answer.missing_data.extend(snapshot.data_warnings.iter().cloned().map(MissingDataItem::from));
            }
        }
        FinanceTaskType::InvestmentReadiness => {
            if let Some(snapshot) = snapshot {
                let has_high_interest_debt = snapshot
                    .liabilities
                    .iter()
                    .any(|l| l.balance_cents > 0 && l.apr_pct.unwrap_or(0.0) >= 8.0);
                let monthly_surplus_cents =
                    snapshot.avg_monthly_income_90d_cents - snapshot.avg_monthly_expense_90d_cents;
                let unstable_cashflow = monthly_surplus_cents <= 0;
                let urgent_goal = urgent_underfunded_goal(snapshot);
                let ready = snapshot.emergency_fund_months >= 1.0
                    && !has_high_interest_debt
                    && !unstable_cashflow
                    && urgent_goal.is_none();
                answer.recommendation = if ready {
                    "You look ready to consider investing from a cashflow, debt, emergency-fund, and near-term-goal standpoint, but this app should keep advice principles-only.".to_string()
                } else {
                    "Do not prioritize investing yet; stabilize cashflow, protect emergency coverage, clear high-interest debt, and fund urgent goals first.".to_string()
                };
                answer.summary = format!(
                    "Emergency coverage is {:.1} month(s); monthly surplus is {}; high-interest debt present: {}; urgent underfunded goal: {}.",
                    snapshot.emergency_fund_months,
                    format_cents(monthly_surplus_cents),
                    has_high_interest_debt,
                    urgent_goal
                        .map(|goal| goal.name.as_str())
                        .unwrap_or("none")
                );
                answer.reasoning = "Investing readiness is based only on local emergency-fund, debt, cashflow, and goal-deadline data, not market research.".to_string();
                if unstable_cashflow {
                    answer.risks.push(
                        "Recent cashflow is not stable enough to prioritize investing.".to_string(),
                    );
                }
                if let Some(goal) = urgent_goal {
                    answer.risks.push(format!(
                        "{} is underfunded for its near-term target date.",
                        goal.name
                    ));
                }
                answer.assumptions.push("No external market data is used; no tickers, ETFs, or market timing are recommended.".to_string());
                answer.missing_data.extend(snapshot.data_warnings.iter().cloned().map(MissingDataItem::from));
            }
        }
        FinanceTaskType::Unknown => {}
    }

    if mentions_investing(question) {
        answer.assumptions.push("Investment advice is principles-only because FinSight does not use external market data in this version.".to_string());
    }

    Ok(answer)
}

fn base_answer(plan: &FinancePlan, evidence: &[EvidenceRecord]) -> StructuredFinanceAnswer {
    let mut data_sources = evidence
        .iter()
        .flat_map(|item| item.evidence.data_sources.clone())
        .collect::<Vec<_>>();
    data_sources.sort();
    data_sources.dedup();
    let mut missing_data = evidence
        .iter()
        .flat_map(|item| item.evidence.missing_data.clone())
        .collect::<Vec<_>>();
    MissingDataItem::dedup(&mut missing_data);
    let numbers_used = evidence
        .iter()
        .flat_map(|item| item.evidence.numbers_used.clone())
        .collect();
    StructuredFinanceAnswer {
        recommendation: String::new(),
        summary: String::new(),
        alternatives: Vec::new(),
        numbers_used,
        data_sources,
        assumptions: Vec::new(),
        missing_data,
        risks: plan.risk_flags.clone(),
        next_actions: Vec::new(),
        what_would_change_recommendation: default_change_conditions(plan),
        confidence: 0.85,
        reasoning: String::new(),
        trace: evidence
            .iter()
            .map(|item| format!("Called tool: {}", item.evidence.tool_name))
            .collect(),
        follow_up_questions: Vec::new(),
        verification: empty_verification(),
    }
}

fn default_change_conditions(plan: &FinancePlan) -> Vec<String> {
    match plan.task_type {
        FinanceTaskType::CashInflowAllocation => vec![
            "The allocation changes if APRs, minimum payments, emergency coverage, or the inflow amount changes."
                .to_string(),
        ],
        FinanceTaskType::GoalEta | FinanceTaskType::GoalAllocation => vec![
            "The goal recommendation changes if the contribution amount, deadline, goal priority, or current balance changes."
                .to_string(),
        ],
        FinanceTaskType::DebtRanking | FinanceTaskType::DebtPayoffScenario => vec![
            "The debt recommendation changes if APRs, balances, minimum payments, or available extra payment changes."
                .to_string(),
        ],
        FinanceTaskType::DebtVsGoal => vec![
            "The savings-versus-debt recommendation changes if emergency coverage, debt APR, minimum payment, or goal urgency changes."
                .to_string(),
        ],
        FinanceTaskType::EmergencyFundPlanning => vec![
            "The emergency-fund target changes if average monthly expenses, liquid cash, or income stability changes."
                .to_string(),
        ],
        FinanceTaskType::CashflowTimeline => vec![
            "The cashflow recommendation changes if paycheck cadence, bill due dates, planned transactions, or average expenses change."
                .to_string(),
        ],
        FinanceTaskType::GoalConflict => vec![
            "The goal-funding recommendation changes if upcoming bills, planned transactions, monthly surplus, emergency floor, or goal contribution amount changes."
                .to_string(),
        ],
        FinanceTaskType::PurchaseAffordability => vec![
            "The affordability recommendation changes if purchase price, emergency cash, monthly surplus, high-interest debt, or planned bills change."
                .to_string(),
        ],
        FinanceTaskType::InvestmentReadiness => vec![
            "The investing-readiness recommendation changes if emergency coverage reaches target, high-interest debt is cleared, or goal deadlines shift."
                .to_string(),
        ],
        FinanceTaskType::DataQualityReport => vec![
            "The data-quality recommendation changes after missing APRs, minimum payments, uncategorized expenses, and stale balances are fixed."
                .to_string(),
        ],
        FinanceTaskType::FinancialSnapshot | FinanceTaskType::GeneralFinancePlanning => vec![
            "The recommendation changes when the underlying balances, transactions, goals, debts, or planned bills change."
                .to_string(),
        ],
        FinanceTaskType::Unknown => Vec::new(),
    }
}

fn blocked_for_missing_inputs(plan: &FinancePlan) -> StructuredFinanceAnswer {
    StructuredFinanceAnswer {
        recommendation: "I need one more detail before I can give a reliable recommendation."
            .to_string(),
        summary: plan.missing_inputs.join(" "),
        alternatives: Vec::new(),
        numbers_used: Vec::new(),
        data_sources: Vec::new(),
        assumptions: Vec::new(),
        missing_data: plan.missing_inputs.iter().cloned().map(MissingDataItem::from).collect(),
        risks: plan.risk_flags.clone(),
        next_actions: Vec::new(),
        what_would_change_recommendation: Vec::new(),
        confidence: 0.0,
        reasoning: "Required prompt inputs were missing.".to_string(),
        trace: Vec::new(),
        follow_up_questions: plan.missing_inputs.clone(),
        verification: empty_verification(),
    }
}

fn verify_answer(
    plan: &FinancePlan,
    evidence: &[EvidenceRecord],
    answer: &StructuredFinanceAnswer,
) -> VerificationReport {
    let mut findings = Vec::new();
    let mut follow_up = Vec::new();
    let mut severity = VerificationSeverity::Ok;

    for tool in &plan.required_tools {
        if !evidence.iter().any(|item| item.evidence.tool_name == *tool) {
            findings.push(format!("Required tool '{tool}' was not run."));
            severity = VerificationSeverity::Blocked;
        }
    }

    if answer.recommendation.trim().is_empty() {
        findings.push("Recommendation is missing.".to_string());
        severity = VerificationSeverity::Blocked;
    }
    if answer.data_sources.is_empty() && plan.task_type != FinanceTaskType::Unknown {
        findings.push("Data sources are missing.".to_string());
        severity = severity.max_warning();
    }
    if answer.what_would_change_recommendation.is_empty()
        && plan.task_type != FinanceTaskType::Unknown
    {
        findings.push("What-would-change-this-recommendation section is missing.".to_string());
        severity = severity.max_warning();
    }
    if matches!(
        plan.task_type,
        FinanceTaskType::DebtVsGoal
            | FinanceTaskType::GoalConflict
            | FinanceTaskType::PurchaseAffordability
    ) && answer.alternatives.len() < 2
    {
        findings.push("This planning answer must compare at least two alternatives.".to_string());
        severity = VerificationSeverity::Blocked;
    }
    if matches!(plan.task_type, FinanceTaskType::InvestmentReadiness)
        && contains_investment_specifics(&format!("{} {}", answer.recommendation, answer.summary))
    {
        findings
            .push("Investment answer contains ticker/ETF/market-timing specificity.".to_string());
        severity = VerificationSeverity::Blocked;
    }

    let critical_missing = critical_missing_data(plan.task_type, &answer.missing_data);
    for missing in &answer.missing_data {
        if is_critical_missing_data(&missing.message) {
            severity = severity.max_warning();
            follow_up.push(missing.message.clone());
        }
    }
    if !critical_missing.is_empty() {
        severity = VerificationSeverity::Blocked;
        findings.push(format!(
            "Critical planning data is missing for {:?}: {}",
            plan.task_type,
            critical_missing.join("; ")
        ));
        follow_up.extend(critical_missing);
    }
    follow_up.sort();
    follow_up.dedup();

    let data_adjustment = data_completeness_confidence_adjustment(answer, &follow_up);
    let severity_adjustment = match severity {
        VerificationSeverity::Ok => 0.0,
        VerificationSeverity::Warning => -0.2,
        VerificationSeverity::Blocked => -0.85,
    };

    VerificationReport {
        passed: severity != VerificationSeverity::Blocked,
        severity,
        findings,
        confidence_adjustment: (severity_adjustment + data_adjustment).max(-0.95),
        required_follow_up_questions: follow_up,
    }
}

fn is_critical_missing_data(missing: &str) -> bool {
    let lower = missing.to_lowercase();
    lower.contains("missing apr")
        || lower.contains("missing minimum payment")
        || lower.contains("need apr")
        || lower.contains("minimum payment")
        || lower.contains("income") && lower.contains("missing")
        || lower.contains("expense") && lower.contains("missing")
}

fn critical_missing_data(task_type: FinanceTaskType, missing_data: &[MissingDataItem]) -> Vec<String> {
    let requires_debt_math = matches!(
        task_type,
        FinanceTaskType::DebtPayoffScenario | FinanceTaskType::DebtVsGoal
    );
    let requires_cashflow_math = matches!(
        task_type,
        FinanceTaskType::CashflowTimeline | FinanceTaskType::EmergencyFundPlanning
    );

    missing_data
        .iter()
        .filter(|item| {
            let lower = item.message.to_lowercase();
            (requires_debt_math
                && (lower.contains("missing apr")
                    || lower.contains("missing minimum payment")
                    || lower.contains("need apr")
                    || lower.contains("need apr and minimum payment")))
                || (requires_cashflow_math
                    && ((lower.contains("income") && lower.contains("missing"))
                        || (lower.contains("expense") && lower.contains("missing"))))
        })
        .map(|item| item.message.clone())
        .collect()
}

fn data_completeness_confidence_adjustment(
    answer: &StructuredFinanceAnswer,
    critical_follow_up: &[String],
) -> f64 {
    let missing_penalty = (answer.missing_data.len() as f64 * -0.04).max(-0.24);
    let follow_up_penalty = (critical_follow_up.len() as f64 * -0.08).max(-0.32);
    let source_bonus = if answer.data_sources.is_empty() {
        -0.08
    } else {
        0.0
    };
    let number_bonus = if answer.numbers_used.is_empty() {
        -0.05
    } else {
        0.0
    };
    missing_penalty + follow_up_penalty + source_bonus + number_bonus
}

fn apply_verification(answer: &mut StructuredFinanceAnswer) {
    answer.confidence =
        (answer.confidence + answer.verification.confidence_adjustment).clamp(0.0, 1.0);
    if answer.verification.severity == VerificationSeverity::Blocked {
        answer.recommendation =
            "I need more reliable data before making that recommendation.".to_string();
        answer
            .follow_up_questions
            .extend(answer.verification.required_follow_up_questions.clone());
    }
}

impl VerificationSeverity {
    fn max_warning(self) -> Self {
        match self {
            VerificationSeverity::Ok => VerificationSeverity::Warning,
            other => other,
        }
    }
}

fn empty_verification() -> VerificationReport {
    VerificationReport {
        passed: true,
        severity: VerificationSeverity::Ok,
        findings: Vec::new(),
        confidence_adjustment: 0.0,
        required_follow_up_questions: Vec::new(),
    }
}

fn snapshot_evidence(snapshot: &FinancialSnapshot) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "get_financial_snapshot".to_string(),
        summary: format!(
            "Liquid balance {}, total balance {}, emergency coverage {:.1} months.",
            format_cents(snapshot.liquid_balance_cents),
            format_cents(snapshot.total_account_balance_cents),
            snapshot.emergency_fund_months
        ),
        data_sources: default_data_sources(),
        missing_data: snapshot.data_warnings.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: vec![
            NumberUsed {
                label: "liquid balance".to_string(),
                value: format_cents(snapshot.liquid_balance_cents),
                source: "accounts/account_balances".to_string(),
            },
            NumberUsed {
                label: "emergency coverage".to_string(),
                value: format!("{:.1} months", snapshot.emergency_fund_months),
                source: "transactions/account_balances".to_string(),
            },
        ],
        raw_json: serde_json::to_value(snapshot)?,
    })
}

fn cash_inflow_evidence(advice: &CashInflowAdvice) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "analyze_cash_inflow".to_string(),
        summary: format!(
            "Allocated {} across {} bucket(s).",
            format_cents(advice.amount_cents),
            advice.allocations.len()
        ),
        data_sources: default_data_sources(),
        missing_data: advice.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: advice
            .allocations
            .iter()
            .map(|allocation| NumberUsed {
                label: allocation.bucket.clone(),
                value: format_cents(allocation.amount_cents),
                source: "analyze_cash_inflow".to_string(),
            })
            .collect(),
        raw_json: serde_json::to_value(advice)?,
    })
}

fn goal_eta_evidence(eta: &GoalEtaResult) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "calculate_goal_eta".to_string(),
        summary: format!("{} ETA is {:?} month(s).", eta.goal_name, eta.eta_months),
        data_sources: vec!["Active goals".to_string()],
        missing_data: Vec::new(),
        numbers_used: vec![
            NumberUsed {
                label: "remaining goal amount".to_string(),
                value: format_cents(eta.remaining_cents),
                source: "goals".to_string(),
            },
            NumberUsed {
                label: "monthly equivalent contribution".to_string(),
                value: format_cents(eta.monthly_equivalent_cents),
                source: "calculate_goal_eta".to_string(),
            },
        ],
        raw_json: serde_json::to_value(eta)?,
    })
}

fn debt_ranking_evidence(ranking: &DebtPayoffRanking) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "rank_debt_payoff".to_string(),
        summary: format!(
            "Ranked {} debt(s) by {}.",
            ranking.items.len(),
            ranking.method
        ),
        data_sources: vec!["Tracked liabilities, APRs, and minimum payments".to_string()],
        missing_data: ranking.missing_data.clone(),
        numbers_used: ranking
            .items
            .iter()
            .map(|item| NumberUsed {
                label: format!("{} balance", item.name),
                value: format_cents(item.balance_cents),
                source: "liabilities".to_string(),
            })
            .collect(),
        raw_json: serde_json::to_value(ranking)?,
    })
}

fn debt_payoff_scenario_evidence(scenarios: &DebtPayoffScenarios) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "run_debt_payoff_scenarios".to_string(),
        summary: format!(
            "Modeled {} payoff with {} extra monthly.",
            scenarios.method,
            format_cents(scenarios.extra_monthly_payment_cents)
        ),
        data_sources: vec!["Tracked liabilities, APRs, and minimum payments".to_string()],
        missing_data: scenarios.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: vec![
            NumberUsed {
                label: "total debt balance".to_string(),
                value: format_cents(scenarios.total_balance_cents),
                source: "liabilities".to_string(),
            },
            NumberUsed {
                label: "extra monthly debt payment".to_string(),
                value: format_cents(scenarios.extra_monthly_payment_cents),
                source: "run_debt_payoff_scenarios".to_string(),
            },
        ],
        raw_json: serde_json::to_value(scenarios)?,
    })
}

fn goal_allocation_evidence(scenarios: &GoalAllocationScenarios) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "run_goal_allocation_scenarios".to_string(),
        summary: format!(
            "Allocated {} monthly across {} goal(s).",
            format_cents(scenarios.monthly_available_cents),
            scenarios.allocations.len()
        ),
        data_sources: vec!["Active goals".to_string()],
        missing_data: scenarios.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: scenarios
            .allocations
            .iter()
            .map(|item| NumberUsed {
                label: format!("{} suggested monthly", item.goal_name),
                value: format_cents(item.suggested_monthly_cents),
                source: "run_goal_allocation_scenarios".to_string(),
            })
            .collect(),
        raw_json: serde_json::to_value(scenarios)?,
    })
}

fn goal_conflict_evidence(scenario: &GoalConflictScenario) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "run_goal_conflict_scenario".to_string(),
        summary: format!(
            "Compared {} goal contribution against {} of upcoming obligations.",
            format_cents(scenario.requested_contribution_cents),
            format_cents(scenario.upcoming_obligations_cents)
        ),
        data_sources: default_data_sources(),
        missing_data: scenario.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: vec![
            NumberUsed {
                label: "requested goal contribution".to_string(),
                value: format_cents(scenario.requested_contribution_cents),
                source: "user prompt".to_string(),
            },
            NumberUsed {
                label: "upcoming obligations".to_string(),
                value: format_cents(scenario.upcoming_obligations_cents),
                source: "recurring bills and planned transactions".to_string(),
            },
            NumberUsed {
                label: "safe contribution now".to_string(),
                value: format_cents(scenario.safe_contribution_now_cents),
                source: "run_goal_conflict_scenario".to_string(),
            },
        ],
        raw_json: serde_json::to_value(scenario)?,
    })
}

fn emergency_fund_evidence(scenarios: &EmergencyFundScenarios) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "run_emergency_fund_scenarios".to_string(),
        summary: format!(
            "Emergency coverage is {:.1} month(s).",
            scenarios.current_months
        ),
        data_sources: default_data_sources(),
        missing_data: scenarios.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: scenarios
            .targets
            .iter()
            .map(|target| NumberUsed {
                label: format!("{} month emergency fund gap", target.target_months),
                value: format_cents(target.gap_cents),
                source: "run_emergency_fund_scenarios".to_string(),
            })
            .collect(),
        raw_json: serde_json::to_value(scenarios)?,
    })
}

fn cashflow_timeline_evidence(timeline: &CashflowTimeline) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "run_cashflow_timeline".to_string(),
        summary: format!(
            "Modeled {} month(s) from starting liquid balance {}.",
            timeline.months.len(),
            format_cents(timeline.starting_liquid_cents)
        ),
        data_sources: default_data_sources(),
        missing_data: timeline.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: timeline
            .months
            .iter()
            .map(|month| NumberUsed {
                label: format!("month {} ending balance", month.month_index),
                value: format_cents(month.ending_balance_cents),
                source: "run_cashflow_timeline".to_string(),
            })
            .collect(),
        raw_json: serde_json::to_value(timeline)?,
    })
}

fn purchase_affordability_evidence(
    scenario: &PurchaseAffordabilityScenario,
) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "run_purchase_affordability".to_string(),
        summary: format!(
            "Modeled {} purchase; affordable now: {}.",
            format_cents(scenario.purchase_amount_cents),
            scenario.affordable_now
        ),
        data_sources: default_data_sources(),
        missing_data: scenario.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: vec![
            NumberUsed {
                label: "purchase amount".to_string(),
                value: format_cents(scenario.purchase_amount_cents),
                source: "user prompt".to_string(),
            },
            NumberUsed {
                label: "emergency floor".to_string(),
                value: format_cents(scenario.emergency_floor_cents),
                source: "run_purchase_affordability".to_string(),
            },
            NumberUsed {
                label: "monthly surplus".to_string(),
                value: format_cents(scenario.monthly_surplus_cents),
                source: "transactions/settings".to_string(),
            },
        ],
        raw_json: serde_json::to_value(scenario)?,
    })
}
fn data_quality_evidence(report: &DataQualityReport) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "get_data_quality_report".to_string(),
        summary: format!(
            "Data quality report found {} warning(s).",
            report.warnings.len()
        ),
        data_sources: report.data_sources.clone(),
        missing_data: report.warnings.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: vec![
            NumberUsed {
                label: "missing APR count".to_string(),
                value: report.missing_apr_count.to_string(),
                source: "get_data_quality_report".to_string(),
            },
            NumberUsed {
                label: "missing minimum payment count".to_string(),
                value: report.missing_min_payment_count.to_string(),
                source: "get_data_quality_report".to_string(),
            },
        ],
        raw_json: serde_json::to_value(report)?,
    })
}

fn debt_vs_goal_evidence(comparison: &DebtGoalComparison) -> anyhow::Result<ToolEvidence> {
    Ok(ToolEvidence {
        tool_name: "compare_debt_vs_goal".to_string(),
        summary: comparison.recommendation.clone(),
        data_sources: default_data_sources(),
        missing_data: comparison.missing_data.iter().cloned().map(MissingDataItem::from).collect(),
        numbers_used: vec![
            NumberUsed {
                label: "goal current savings".to_string(),
                value: format_cents(comparison.goal_current_cents),
                source: "goals".to_string(),
            },
            NumberUsed {
                label: "compared debt".to_string(),
                value: format_cents(comparison.compared_debt_cents),
                source: "liabilities".to_string(),
            },
        ],
        raw_json: serde_json::to_value(comparison)?,
    })
}

fn default_data_sources() -> Vec<String> {
    vec![
        "Accounts and latest account balances".to_string(),
        "Transactions over the last 90 and 365 days".to_string(),
        "Active goals".to_string(),
        "Tracked liabilities, APRs, and minimum payments".to_string(),
        "Detected recurring bills and planned transactions".to_string(),
    ]
}

fn find_best_goal_match<'a>(
    question: &str,
    goals: &'a [finance::SnapshotGoal],
) -> Option<&'a finance::SnapshotGoal> {
    let q = normalize_name(question);
    goals
        .iter()
        .find(|goal| {
            let name = normalize_name(&goal.name);
            !name.is_empty() && q.contains(&name)
        })
        .or_else(|| {
            goals.iter().find(|goal| {
                let name = normalize_name(&goal.name);
                !name.is_empty() && name.split_whitespace().any(|token| q.contains(token))
            })
        })
}

fn find_best_liability_match<'a>(
    question: &str,
    liabilities: &'a [finance::SnapshotLiability],
) -> Option<&'a finance::SnapshotLiability> {
    let q = normalize_name(question);
    liabilities
        .iter()
        .find(|liability| {
            let name = normalize_name(&liability.name);
            !name.is_empty() && q.contains(&name)
        })
        .or_else(|| {
            liabilities.iter().find(|liability| {
                let name = normalize_name(&liability.name);
                !name.is_empty() && name.split_whitespace().any(|token| q.contains(token))
            })
        })
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn mentions_investing(question: &str) -> bool {
    let q = question.to_lowercase();
    [
        "invest",
        "stocks",
        "stock",
        "etf",
        "ticker",
        "portfolio",
        "voo",
        "vti",
        "spy",
        "qqq",
    ]
    .iter()
    .any(|term| q.contains(term))
}

fn contains_investment_specifics(content: &str) -> bool {
    let lower = content.to_lowercase();
    ["voo", "vti", "spy", "qqq", "buy now", "market timing"]
        .iter()
        .any(|term| lower.contains(term))
}

fn ceil_div(n: i64, d: i64) -> i64 {
    if d <= 0 {
        return 0;
    }
    (n + d - 1) / d
}
fn urgent_underfunded_goal(snapshot: &FinancialSnapshot) -> Option<&finance::SnapshotGoal> {
    snapshot.goals.iter().find(|goal| {
        let Some(target_date) = goal.target_date.as_deref().and_then(parse_goal_date) else {
            return false;
        };
        let months_until_due = ((target_date - Utc::now().date_naive()).num_days() / 30).max(0);
        if !(0..=12).contains(&months_until_due) {
            return false;
        }
        let required_monthly_cents = if months_until_due <= 0 {
            goal.remaining_cents
        } else {
            ceil_div(goal.remaining_cents, months_until_due)
        };
        goal.remaining_cents > 0 && required_monthly_cents > goal.monthly_cents.max(0)
    })
}

fn parse_goal_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").ok())
}
fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let value = cents.abs() as f64 / 100.0;
    if value.fract().abs() < 0.005 {
        format!("{sign}${value:.0}")
    } else {
        format!("{sign}${value:.2}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("planning.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut Connection) {
        conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at) VALUES('a1','Me','Bank','Checking','Checking','USD','#fff',datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id, as_of_date, balance_cents) VALUES('a1','2026-06-01',500000)", []).unwrap();
        conn.execute("INSERT INTO goals(id,name,type,target_cents,current_cents,monthly_cents,color,sort_order,created_at) VALUES('car','Car','save-by-date',2000000,500000,50000,'#fff',0,datetime('now'))", []).unwrap();
        // Debt is now a Credit/Loan-type Account with a negative balance, not
        // a separate liabilities-table row.
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,limit_cents,created_at) VALUES('cc','Household','Manual','Credit','Credit Card','USD','#F97316','manual','restricted',0,'debt',24.9,5000,500000,datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('cc',date('now'),-250000,'manual')", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,created_at) VALUES('loan','Household','Manual','Loan','Loan','USD','#F87171','manual','restricted',0,'debt',5.0,30000,datetime('now'))", []).unwrap();
        conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('loan',date('now'),-1800000,'manual')", []).unwrap();
        for days in [10, 40, 70] {
            conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES(hex(randomblob(16)),'a1',datetime('now', ?1),300000,'Payroll','cleared',datetime('now'))", [format!("-{days} days")]).unwrap();
        }
        for days in [5, 35, 65] {
            conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES(hex(randomblob(16)),'a1',datetime('now', ?1),-200000,'Rent','cleared',datetime('now'))", [format!("-{days} days")]).unwrap();
        }
    }

    struct HouseholdFixture {
        _dir: TempDir,
        db: Db,
    }

    fn representative_household(profile: &str) -> HouseholdFixture {
        let (_dir, db) = fresh();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn);
            match profile {
                "starter" => {
                    conn.execute("UPDATE account_balances SET balance_cents = 150000 WHERE account_id = 'a1'", []).unwrap();
                    conn.execute(
                        "UPDATE account_balances SET balance_cents = 0 WHERE account_id IN ('cc', 'loan')",
                        [],
                    )
                    .unwrap();
                    conn.execute("UPDATE goals SET current_cents = 50000, monthly_cents = 25000 WHERE id = 'car'", []).unwrap();
                }
                "debt_heavy" => {}
                "goal_pressure" => {
                    let target_date =
                        (Utc::now().date_naive() + chrono::Duration::days(90)).to_string();
                    conn.execute(
                        "UPDATE goals SET target_date = ?1, monthly_cents = 50000 WHERE id = 'car'",
                        [target_date],
                    )
                    .unwrap();
                    conn.execute(
                        "INSERT INTO planned_transactions(id, description, amount_cents, account_id, due_date, status, source, created_at) VALUES('fixture_bill','Insurance premium',-300000,'a1',date('now','+7 days'),'planned','manual',datetime('now'))",
                        [],
                    )
                    .unwrap();
                }
                other => panic!("unknown household fixture: {other}"),
            }
        }
        HouseholdFixture { _dir, db }
    }

    #[test]
    fn representative_household_fixtures_cover_planning_modes() {
        let cases = [
            (
                "starter",
                "I got paid $1,000. What should I do?",
                "analyze_cash_inflow",
            ),
            (
                "debt_heavy",
                "How long to pay off my debt with an extra $500 monthly?",
                "run_debt_payoff_scenarios",
            ),
            (
                "goal_pressure",
                "Can I put $1,000 into my car goal with upcoming bills?",
                "run_goal_conflict_scenario",
            ),
        ];

        for (profile, question, expected_tool) in cases {
            let fixture = representative_household(profile);
            let mut conn = fixture.db.get().unwrap();
            let answer = answer_finance_question(&mut conn, question)
                .unwrap()
                .unwrap();
            assert!(
                answer
                    .trace
                    .iter()
                    .any(|trace| trace.contains(expected_tool)),
                "{profile} did not call {expected_tool}: {:?}",
                answer.trace
            );
        }
    }
    #[test]
    fn planner_selects_cash_inflow_tools() {
        let plan = plan_finance_question("I got paid $3,000. What should I do?");
        assert_eq!(plan.task_type, FinanceTaskType::CashInflowAllocation);
        assert!(plan
            .required_tools
            .contains(&"analyze_cash_inflow".to_string()));
        assert!(plan
            .required_tools
            .contains(&"get_financial_snapshot".to_string()));
    }

    #[test]
    fn planner_selects_goal_eta_tool() {
        let plan = plan_finance_question("If I save $500 biweekly, when do I reach my car goal?");
        assert_eq!(plan.task_type, FinanceTaskType::GoalEta);
        assert!(plan
            .required_tools
            .contains(&"calculate_goal_eta".to_string()));
    }

    #[test]
    fn planner_selects_new_scenario_tools() {
        let debt = plan_finance_question("How long to pay off my debt with an extra $500 monthly?");
        assert_eq!(debt.task_type, FinanceTaskType::DebtPayoffScenario);
        assert!(debt
            .required_tools
            .contains(&"run_debt_payoff_scenarios".to_string()));

        let emergency = plan_finance_question("How much emergency fund do I need?");
        assert_eq!(emergency.task_type, FinanceTaskType::EmergencyFundPlanning);
        assert!(emergency
            .required_tools
            .contains(&"run_emergency_fund_scenarios".to_string()));

        let cashflow = plan_finance_question("Will my end of month balance get too low?");
        assert_eq!(cashflow.task_type, FinanceTaskType::CashflowTimeline);
        assert!(cashflow
            .required_tools
            .contains(&"run_cashflow_timeline".to_string()));

        let quality = plan_finance_question("What missing data should I fix?");
        assert_eq!(quality.task_type, FinanceTaskType::DataQualityReport);
        assert!(quality
            .required_tools
            .contains(&"get_data_quality_report".to_string()));
    }

    #[test]
    fn planner_sets_investment_guardrail() {
        let plan = plan_finance_question("Should I invest in stocks or ETFs?");
        assert_eq!(plan.task_type, FinanceTaskType::InvestmentReadiness);
        assert!(plan
            .risk_flags
            .contains(&"investment_principles_only".to_string()));
    }

    #[test]
    fn verifier_blocks_debt_vs_goal_without_alternatives() {
        let plan = FinancePlan {
            task_type: FinanceTaskType::DebtVsGoal,
            required_tools: vec![],
            optional_tools: vec![],
            required_inputs: vec![],
            missing_inputs: vec![],
            planning_notes: vec![],
            risk_flags: vec![],
        };
        let answer = StructuredFinanceAnswer {
            recommendation: "Pay debt.".to_string(),
            summary: String::new(),
            alternatives: Vec::new(),
            numbers_used: Vec::new(),
            data_sources: vec!["liabilities".to_string()],
            assumptions: Vec::new(),
            missing_data: Vec::new(),
            risks: Vec::new(),
            next_actions: Vec::new(),
            what_would_change_recommendation: Vec::new(),
            confidence: 0.8,
            reasoning: String::new(),
            trace: Vec::new(),
            follow_up_questions: Vec::new(),
            verification: empty_verification(),
        };
        let report = verify_answer(&plan, &[], &answer);
        assert_eq!(report.severity, VerificationSeverity::Blocked);
    }

    #[test]
    fn answers_car_savings_vs_loan_with_alternatives() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(
            &mut conn,
            "Should I use my car savings to pay off a similar-sized loan?",
        )
        .unwrap()
        .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer.alternatives.len() >= 2);
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("compare_debt_vs_goal")));
    }

    #[test]
    fn structured_answer_snapshot_includes_required_fields() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(
            &mut conn,
            "Should I use my car savings to pay off a similar-sized loan?",
        )
        .unwrap()
        .unwrap();

        let snapshot = serde_json::to_value(&answer).unwrap();
        let object = snapshot.as_object().unwrap();
        for key in [
            "recommendation",
            "summary",
            "alternatives",
            "numbers_used",
            "data_sources",
            "assumptions",
            "missing_data",
            "next_actions",
            "what_would_change_recommendation",
            "trace",
            "verification",
        ] {
            assert!(
                object.contains_key(key),
                "missing structured answer field: {key}"
            );
        }
        assert!(snapshot["alternatives"].as_array().unwrap().len() >= 2);
        assert!(!snapshot["data_sources"].as_array().unwrap().is_empty());
        assert!(!snapshot["what_would_change_recommendation"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(snapshot["verification"]["severity"], "ok");
    }
    #[test]
    fn answer_includes_what_would_change_recommendation() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(&mut conn, "How much emergency fund do I need?")
            .unwrap()
            .unwrap();

        assert!(!answer.what_would_change_recommendation.is_empty());
        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
    }

    #[test]
    fn answers_semimonthly_goal_eta() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(
            &mut conn,
            "If I save $500 semi-monthly, when do I reach my car goal?",
        )
        .unwrap()
        .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("calculate_goal_eta")));
        assert!(answer.recommendation.contains("semimonthly"));
        assert!(answer.summary.contains("$1000"));
        assert!(answer.summary.contains("$15000") || answer.summary.contains("$15,000"));
    }

    #[test]
    fn missing_debt_math_data_blocks_payoff_timeline_confidence() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "UPDATE accounts SET apr_pct = NULL, min_payment_cents = NULL WHERE id = 'loan'",
            [],
        )
        .unwrap();

        let answer = answer_finance_question(
            &mut conn,
            "How long to pay off my debt with an extra $500 monthly?",
        )
        .unwrap()
        .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Blocked);
        assert!(answer.confidence <= 0.1);
        assert!(answer
            .verification
            .findings
            .iter()
            .any(|finding| finding.contains("Critical planning data is missing")));
        assert!(answer
            .follow_up_questions
            .iter()
            .any(|question| question.to_lowercase().contains("apr")));
        assert_eq!(
            answer.recommendation,
            "I need more reliable data before making that recommendation."
        );
    }

    #[test]
    fn answers_debt_payoff_timeline_with_scenario_tool() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(
            &mut conn,
            "How long to pay off my debt with an extra $500 monthly?",
        )
        .unwrap()
        .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("run_debt_payoff_scenarios")));
        assert!(answer.summary.contains("saving about"));
    }

    #[test]
    fn answers_large_purchase_affordability_with_alternatives() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(&mut conn, "Can I afford a $4,500 purchase?")
            .unwrap()
            .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("run_purchase_affordability")));
        assert!(
            answer.recommendation.contains("Delay") || answer.recommendation.contains("affordable")
        );
        assert!(answer.summary.contains("$4500") || answer.summary.contains("$4,500"));
        assert!(answer.alternatives.len() >= 3);
        assert!(answer
            .next_actions
            .iter()
            .any(|item| item.contains("wait/save")));
    }
    #[test]
    fn answers_goal_conflict_with_upcoming_bills() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        conn.execute(
            "INSERT INTO planned_transactions(id, description, amount_cents, account_id, due_date, status, source, created_at) VALUES('bill1','Insurance premium',-350000,'a1',date('now','+7 days'),'planned','manual',datetime('now'))",
            [],
        )
        .unwrap();

        let answer = answer_finance_question(
            &mut conn,
            "Can I put $1,000 into my car goal with upcoming bills?",
        )
        .unwrap()
        .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("run_goal_conflict_scenario")));
        assert!(answer.recommendation.contains("Delay") || answer.recommendation.contains("safe"));
        assert!(answer.summary.contains("upcoming obligations"));
        assert!(answer.alternatives.len() >= 3);
        assert!(answer
            .risks
            .iter()
            .any(|risk| risk.contains("upcoming bills")));
    }

    #[test]
    fn answers_emergency_fund_with_scenario_tool() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(&mut conn, "How much emergency fund do I need?")
            .unwrap()
            .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .trace
            .iter()
            .any(|t| t.contains("run_emergency_fund_scenarios")));
        assert!(answer.summary.contains("six-month") || answer.summary.contains("6 mo"));
    }

    #[test]
    fn investment_answer_stays_principles_only() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let answer = answer_finance_question(&mut conn, "Should I buy VOO or QQQ now?")
            .unwrap()
            .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .assumptions
            .iter()
            .any(|item| item.contains("principles-only")));
        assert!(!answer.recommendation.contains("VOO"));
        assert!(!answer.recommendation.contains("QQQ"));
    }

    #[test]
    fn investment_answer_blocks_unstable_cashflow_and_urgent_goal() {
        let (_dir, db) = fresh();
        let mut conn = db.get().unwrap();
        seed(&mut conn);
        let target_date = (Utc::now().date_naive() + chrono::Duration::days(90)).to_string();
        conn.execute(
            "UPDATE accounts SET apr_pct = 3.0 WHERE id IN ('cc', 'loan')",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE goals SET target_date = ?1, monthly_cents = 50000 WHERE id = 'car'",
            [target_date],
        )
        .unwrap();
        for days in [3, 33, 63] {
            conn.execute("INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES(hex(randomblob(16)),'a1',datetime('now', ?1),-200000,'Extra costs','cleared',datetime('now'))", [format!("-{days} days")]).unwrap();
        }

        let answer = answer_finance_question(&mut conn, "Should I invest in stocks or ETFs?")
            .unwrap()
            .unwrap();

        assert_eq!(answer.verification.severity, VerificationSeverity::Ok);
        assert!(answer
            .recommendation
            .contains("Do not prioritize investing yet"));
        assert!(answer.summary.contains("monthly surplus is -$1000"));
        assert!(answer.summary.contains("urgent underfunded goal: Car"));
        assert!(answer.risks.iter().any(|risk| risk.contains("cashflow")));
        assert!(answer.risks.iter().any(|risk| risk.contains("underfunded")));
    }
}
