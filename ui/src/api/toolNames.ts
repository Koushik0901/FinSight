// Mirror of finsight-agent's `reasoning::tools::names::ALL_TOOL_NAMES`
// (crates/finsight-agent/src/reasoning/tools/mod.rs), in the same order.
// Single source of truth is the Rust const list: to add/rename a tool, edit it
// there and re-mirror here. `crates/finsight-server/tests/parity.rs` fails if
// this file ever drifts, and `toolNames.test.ts` pins the length so a quiet
// append without a Rust const is caught on the frontend side too.
//
// No per-tool named exports: 45 constants nobody imports is weight — consumers
// need the set, not individual names.
export const BACKEND_TOOL_NAMES = [
  "get_financial_snapshot",
  "analyze_cash_inflow",
  "calculate_goal_eta",
  "rank_debt_payoff",
  "compare_payoff_strategies",
  "get_counterparty_position",
  "plan_sinking_funds",
  "compare_debt_vs_goal",
  "get_account_balances",
  "get_account_balance_history",
  "get_net_worth",
  "explain_metric",
  "explain_basis",
  "reconcile_bases",
  "get_safe_to_spend",
  "list_saved_scenarios",
  "get_month_totals",
  "get_top_spending_categories",
  "get_spending_breakdown",
  "get_member_spending",
  "get_budgets",
  "get_goals",
  "get_recurring_bills",
  "get_liabilities",
  "search_transactions",
  "find_anomalies",
  "list_uncategorized_transactions",
  "run_cashflow_projection",
  "run_debt_payoff_scenarios",
  "run_goal_allocation_scenarios",
  "run_goal_conflict_scenario",
  "run_emergency_fund_scenarios",
  "run_cashflow_timeline",
  "run_purchase_affordability",
  "get_data_quality_report",
  "explain_spending_change",
  "classify_spending_period",
  "annotate_spending_driver",
  "plan_spending_reduction",
  "draft_set_budget",
  "draft_update_goal_monthly",
  "draft_create_planned_transaction",
  "draft_save_scenario",
  "draft_debt_payoff_plan",
  "draft_recategorization",
] as const;
export type BackendToolName = (typeof BACKEND_TOOL_NAMES)[number];
