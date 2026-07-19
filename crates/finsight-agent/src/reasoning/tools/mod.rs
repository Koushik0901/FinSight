pub mod act;
pub mod read;
pub mod spending;

use crate::reasoning::messages::{AgentDraftAction, ToolDefinition};
use anyhow::Result;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    fn execute(&self, ctx: &mut ToolContext, args: Value) -> Result<Value>;
}

pub struct ToolContext<'a> {
    pub conn: &'a mut Connection,
    pub changes: &'a mut Vec<crate::reasoning::messages::AgentChange>,
    pub draft_actions: &'a mut Vec<AgentDraftAction>,
}

#[derive(Debug, Clone)]
pub struct ToolExecutionError {
    pub tool_name: String,
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

impl ToolExecutionError {
    pub fn to_tool_result(&self) -> Value {
        json!({
            "ok": false,
            "error": {
                "tool_name": self.tool_name,
                "code": self.code,
                "message": self.message,
                "retryable": self.retryable,
            }
        })
    }
}

pub struct ToolExecutionResult {
    pub value: Value,
    pub had_error: bool,
}

pub struct ToolSet {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl Default for ToolSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolSet {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|t| ToolDefinition {
                name: t.name().to_string(),
                description: t.description().to_string(),
                parameters: t.parameters(),
            })
            .collect()
    }
    pub fn execute(&self, name: &str, ctx: &mut ToolContext, args: Value) -> Result<Value> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", name))?;
        tool.execute(ctx, args)
    }

    pub fn execute_recoverable(
        &self,
        name: &str,
        ctx: &mut ToolContext,
        args: Value,
    ) -> ToolExecutionResult {
        match self.try_execute(name, ctx, args) {
            Ok(mut value) => {
                // Give the model a formatted dollar string next to every raw
                // `_cents` integer so it can quote the value verbatim instead of
                // dividing by 100 in its head — a step it gets wrong ~10-15% of
                // the time (dropping a zero: $7,000 -> $700).
                augment_cents_fields(&mut value);
                ToolExecutionResult {
                    value: json!({"ok": true, "data": value}),
                    had_error: false,
                }
            }
            Err(error) => ToolExecutionResult {
                value: error.to_tool_result(),
                had_error: true,
            },
        }
    }

    fn try_execute(
        &self,
        name: &str,
        ctx: &mut ToolContext,
        args: Value,
    ) -> std::result::Result<Value, ToolExecutionError> {
        let Some(tool) = self.tools.get(name) else {
            return Err(ToolExecutionError {
                tool_name: name.to_string(),
                code: "unknown_tool".to_string(),
                message: format!(
                    "Unknown tool '{name}'. Choose one of the tools listed in the system prompt."
                ),
                retryable: true,
            });
        };
        validate_tool_arguments(name, &tool.parameters(), &args)?;
        tool.execute(ctx, args).map_err(|err| ToolExecutionError {
            tool_name: name.to_string(),
            code: "tool_execution_failed".to_string(),
            message: friendly_tool_error(name, &err.to_string()),
            retryable: true,
        })
    }
}

/// Formats integer cents as a signed dollar string with thousands separators,
/// e.g. `-220000 -> "-$2,200.00"`, `700000 -> "$7,000.00"`, `0 -> "$0.00"`.
pub fn format_dollars(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let dollars = abs / 100;
    let rem = abs % 100;
    let digits = dollars.to_string();
    let n = digits.len();
    let mut grouped = String::with_capacity(n + n / 3);
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (n - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}${}.{:02}", if neg { "-" } else { "" }, grouped, rem)
}

/// Recursively adds a `<name>_display` formatted-dollar string next to every
/// integer `<name>_cents` field in a tool result, so the model can quote the
/// dollar value verbatim instead of dividing cents by 100 itself.
pub fn augment_cents_fields(v: &mut Value) {
    match v {
        Value::Object(map) => {
            let additions: Vec<(String, String)> = map
                .iter()
                .filter_map(|(k, val)| {
                    let stem = k.strip_suffix("_cents")?;
                    let c = val.as_i64()?;
                    Some((format!("{stem}_display"), format_dollars(c)))
                })
                .collect();
            for (key, disp) in additions {
                map.entry(key).or_insert(Value::String(disp));
            }
            for val in map.values_mut() {
                augment_cents_fields(val);
            }
        }
        Value::Array(arr) => arr.iter_mut().for_each(augment_cents_fields),
        _ => {}
    }
}

/// The canonical set of tools the Copilot runs with. Single source of truth so
/// the shipped app (`finsight-app::commands::agent::build_toolset`) and the
/// offline evaluation harness (`finsight-eval`) exercise exactly the same
/// capabilities — otherwise the benchmark would grade a different agent than
/// users get.
pub fn standard_toolset() -> ToolSet {
    let mut tools = ToolSet::new();
    tools.register(read::get_financial_snapshot());
    tools.register(read::analyze_cash_inflow());
    tools.register(read::calculate_goal_eta());
    tools.register(read::rank_debt_payoff());
    tools.register(read::compare_debt_vs_goal());
    tools.register(read::get_account_balances());
    tools.register(read::get_account_balance_history());
    tools.register(read::get_net_worth());
    tools.register(read::get_month_totals());
    tools.register(read::get_top_spending_categories());
    tools.register(read::get_spending_breakdown());
    tools.register(read::get_member_spending());
    tools.register(read::get_budgets());
    tools.register(read::get_goals());
    tools.register(read::get_recurring_bills());
    tools.register(read::get_liabilities());
    tools.register(read::search_transactions());
    tools.register(read::find_anomalies());
    tools.register(read::list_uncategorized_transactions());
    tools.register(read::run_cashflow_projection());
    tools.register(read::run_debt_payoff_scenarios());
    tools.register(read::run_goal_allocation_scenarios());
    tools.register(read::run_goal_conflict_scenario());
    tools.register(read::run_emergency_fund_scenarios());
    tools.register(read::run_cashflow_timeline());
    tools.register(read::run_purchase_affordability());
    tools.register(read::get_data_quality_report());
    tools.register(spending::explain_spending_change());
    tools.register(spending::classify_spending_period());
    tools.register(spending::annotate_spending_driver());
    tools.register(spending::plan_spending_reduction());
    tools.register(act::set_budget());
    tools.register(act::update_goal_monthly());
    tools.register(act::create_planned_transaction());
    tools.register(act::save_scenario());
    tools.register(act::create_debt_payoff_plan());
    tools.register(act::draft_recategorization());
    tools
}

fn validate_tool_arguments(
    tool_name: &str,
    schema: &Value,
    args: &Value,
) -> std::result::Result<(), ToolExecutionError> {
    let Some(obj) = args.as_object() else {
        return Err(ToolExecutionError {
            tool_name: tool_name.to_string(),
            code: "invalid_arguments".to_string(),
            message: "Tool arguments must be a JSON object.".to_string(),
            retryable: true,
        });
    };

    if let Some(required) = schema.get("required").and_then(Value::as_array) {
        for key in required.iter().filter_map(Value::as_str) {
            if !obj.contains_key(key) || obj.get(key).is_some_and(Value::is_null) {
                return Err(ToolExecutionError {
                    tool_name: tool_name.to_string(),
                    code: "missing_required_argument".to_string(),
                    message: format!("Missing required argument '{key}'."),
                    retryable: true,
                });
            }
        }
    }

    let Some(properties) = schema.get("properties").and_then(Value::as_object) else {
        return Ok(());
    };

    for (key, value) in obj {
        let Some(prop_schema) = properties.get(key) else {
            return Err(ToolExecutionError {
                tool_name: tool_name.to_string(),
                code: "unknown_argument".to_string(),
                message: format!("Unknown argument '{key}' for tool '{tool_name}'."),
                retryable: true,
            });
        };
        if let Some(expected_type) = prop_schema.get("type").and_then(Value::as_str) {
            let ok = match expected_type {
                "integer" => value.as_i64().is_some(),
                "number" => value.as_f64().is_some(),
                "string" => value.as_str().is_some(),
                "boolean" => value.as_bool().is_some(),
                "object" => value.as_object().is_some(),
                "array" => value.as_array().is_some(),
                _ => true,
            };
            if !ok {
                return Err(ToolExecutionError {
                    tool_name: tool_name.to_string(),
                    code: "invalid_argument_type".to_string(),
                    message: format!("Argument '{key}' must be {expected_type}."),
                    retryable: true,
                });
            }
        }
        if let Some(allowed) = prop_schema.get("enum").and_then(Value::as_array) {
            if !allowed.iter().any(|item| item == value) {
                let options = allowed
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(ToolExecutionError {
                    tool_name: tool_name.to_string(),
                    code: "invalid_argument_value".to_string(),
                    message: format!("Argument '{key}' must be one of: {options}."),
                    retryable: true,
                });
            }
        }
    }

    Ok(())
}

fn friendly_tool_error(tool_name: &str, raw: &str) -> String {
    if raw.contains("QueryReturnedNoRows") || raw.contains("query returned no rows") {
        return format!(
            "{tool_name} could not find the requested record. Re-check the ID with a read tool, then retry."
        );
    }
    if raw.contains("required") {
        return format!("{tool_name} is missing a required input: {raw}");
    }
    raw.to_string()
}

#[cfg(test)]
mod format_tests {
    use super::{augment_cents_fields, format_dollars};
    use serde_json::json;

    #[test]
    fn formats_dollars_with_sign_and_separators() {
        assert_eq!(format_dollars(0), "$0.00");
        assert_eq!(format_dollars(700_000), "$7,000.00");
        assert_eq!(format_dollars(-220_000), "-$2,200.00");
        assert_eq!(format_dollars(-920_000), "-$9,200.00");
        assert_eq!(format_dollars(5000), "$50.00");
        assert_eq!(format_dollars(199), "$1.99");
        assert_eq!(format_dollars(141_301_300), "$1,413,013.00");
    }

    #[test]
    fn augments_nested_cents_fields() {
        let mut v = json!({
            "net_worth_cents": -220000,
            "accounts": [{"name": "Checking", "balance_cents": 200000}],
            "note": "hi"
        });
        augment_cents_fields(&mut v);
        assert_eq!(v["net_worth_display"], "-$2,200.00");
        assert_eq!(v["accounts"][0]["balance_display"], "$2,000.00");
        // Non-cents fields are untouched; raw cents remain for any consumer.
        assert_eq!(v["note"], "hi");
        assert_eq!(v["net_worth_cents"], -220000);
    }
}
