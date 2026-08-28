use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

/// How to slice the expense pie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
pub enum SplitBy {
    Category,
    Group,
    Payee,
    Account,
    Month,
}

impl Default for SplitBy {
    fn default() -> Self {
        Self::Category
    }
}

/// Lookback window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
pub enum Period {
    #[serde(alias = "Last1M", alias = "last1Month", alias = "last1m")]
    Last1Month,
    #[serde(alias = "Last3M", alias = "last3Months", alias = "last3m")]
    Last3Months,
    #[serde(alias = "Last6M", alias = "last6Months", alias = "last6m")]
    Last6Months,
    #[serde(alias = "ytd", alias = "Ytd")]
    YTD,
    #[serde(alias = "AllTime", alias = "all", alias = "allTime")]
    All,
}

impl Default for Period {
    fn default() -> Self {
        Self::All
    }
}

/// Parameters for the custom report query — filters + split.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CustomReportParams {
    #[serde(default, alias = "split_by")]
    pub split_by: SplitBy,
    #[serde(default, alias = "period")]
    pub period: Period,
    #[serde(default, alias = "include_transfers")]
    pub include_transfers: bool,
    #[serde(default, alias = "include_archived")]
    pub include_archived: bool,
}

impl Default for CustomReportParams {
    fn default() -> Self {
        Self {
            split_by: SplitBy::default(),
            period: Period::default(),
            include_transfers: false,
            include_archived: false,
        }
    }
}

/// One grouped row.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ReportRow {
    pub label: String,
    pub total_cents: i64,
    pub txn_count: i64,
}

/// Full result: grouped rows + grand total.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CustomReportResult {
    pub rows: Vec<ReportRow>,
    pub total_cents: i64,
}
