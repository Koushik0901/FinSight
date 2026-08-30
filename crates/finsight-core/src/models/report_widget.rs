use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

/// One customizable Reports canvas widget — vertical-stack reorderable.
///
/// Mirrors Actual's freeform report widget: pick any slice of the ledger
/// (`split_by` + `period` + `filters_json`) and render it as any `chart_type`.
/// Persisted per-user in the encrypted DB (table `report_widgets` V067).
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ReportWidget {
    pub id: String,
    /// 0-based display order. Canonical order is `ORDER BY position ASC, id ASC`.
    pub position: i64,
    pub title: String,
    /// 'table' | 'bar' | 'barStacked' | 'line' | 'area' | 'donut'
    pub chart_type: String,
    /// 'category' | 'group' | 'payee' | 'account' | 'month' | 'spendingType'
    pub split_by: String,
    /// 'Last1Month' | 'Last3Months' | 'Last6Months' | 'YTD' | 'All'
    pub period: String,
    /// JSON object with widget filters: `{ includeTransfers,bool, includeArchived,bool,
    ///   accountIds: string[], categoryIds: string[], groupIds: string[], payee?: string,
    ///   spendingType?: string, memberId?: string|null }`
    /// Stored verbatim; parsed only when executing the widget query.
    pub filters_json: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight filters stored inside `filters_json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct WidgetFilters {
    #[serde(default)]
    pub include_transfers: bool,
    #[serde(default)]
    pub include_archived: bool,
    #[serde(default)]
    pub account_ids: Vec<String>,
    #[serde(default)]
    pub category_ids: Vec<String>,
    #[serde(default)]
    pub group_ids: Vec<String>,
    #[serde(default)]
    pub payee: Option<String>,
    #[serde(default)]
    pub spending_type: Option<String>,
    #[serde(default)]
    pub member_id: Option<String>,
}

/// Request for `POST /api/rpc/create_report_widget`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreateReportWidgetRequest {
    pub title: String,
    pub chart_type: String,
    pub split_by: String,
    pub period: String,
    /// Optional filters JSON. When absent/empty, defaults to `{}`.
    pub filters_json: Option<String>,
    /// Optional explicit position. When absent, appends at end.
    pub position: Option<i64>,
}

/// Request for `POST /api/rpc/update_report_widget`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct UpdateReportWidgetRequest {
    pub id: String,
    pub title: Option<String>,
    pub chart_type: Option<String>,
    pub split_by: Option<String>,
    pub period: Option<String>,
    pub filters_json: Option<String>,
}

/// Request for `POST /api/rpc/delete_report_widget`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DeleteReportWidgetRequest {
    pub id: String,
}

/// Request for `POST /api/rpc/reorder_report_widgets`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ReorderReportWidgetsRequest {
    /// Ordered widget ids, first = position 0.
    pub ordered_ids: Vec<String>,
}

/// K8s-ish validation helpers — called from repo before write.
/// Keep in sync with `SplitBy`/`Period` enums; `validate_split_by`/`validate_period`
/// use serde deserialization as single source of truth so aliases stay valid.
pub const CHART_TYPES: &[&str] = &["table", "bar", "barStacked", "line", "area", "donut"];
pub const SPLIT_BYS: &[&str] = &[
    "category",
    "group",
    "payee",
    "account",
    "month",
    "spendingType",
];
pub const PERIODS: &[&str] = &[
    "Last1Month",
    "Last3Months",
    "Last6Months",
    "YTD",
    "All",
];

pub fn validate_chart_type(v: &str) -> bool {
    CHART_TYPES.contains(&v)
}
pub fn validate_split_by(v: &str) -> bool {
    // Single source: try to deserialize as SplitBy (handles camelCase + aliases)
    serde_json::from_str::<crate::models::SplitBy>(&format!("\"{}\"", v)).is_ok()
}
pub fn validate_period(v: &str) -> bool {
    serde_json::from_str::<crate::models::Period>(&format!("\"{}\"", v)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_bys_and_periods_match_enum() {
        for s in SPLIT_BYS {
            assert!(validate_split_by(s), "SPLIT_BYS entry {} should be valid SplitBy", s);
        }
        for p in PERIODS {
            assert!(validate_period(p), "PERIODS entry {} should be valid Period", p);
        }
        let cases = [
            (crate::models::SplitBy::Category, "category"),
            (crate::models::SplitBy::Group, "group"),
            (crate::models::SplitBy::Payee, "payee"),
            (crate::models::SplitBy::Account, "account"),
            (crate::models::SplitBy::Month, "month"),
            (crate::models::SplitBy::SpendingType, "spendingType"),
        ];
        for (variant, expected) in cases {
            let s = serde_json::to_value(variant).unwrap().as_str().unwrap().to_string();
            assert_eq!(s, expected);
            assert!(SPLIT_BYS.contains(&s.as_str()));
        }
    }
}
