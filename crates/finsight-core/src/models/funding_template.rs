use serde::{Deserialize, Serialize};
use specta::Type;
use utoipa::ToSchema;

/// One funding template — declarative rule for how to fund a category.
///
/// Mirrors Actual's `#template` comment syntax as a table: ordered by
/// `priority` ASC then `id` ASC, evaluated as `need.min(available)` where
/// `available = to_budget(month)` (respects holds). Each kind's `params_json`
/// holds variant-specific fields, e.g. `{"amount":7299}` for `fixed` or
/// `{"cap":30000}` for `up_to`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct FundingTemplate {
    pub id: String,
    /// Category to fund.
    pub category_id: String,
    /// 'fixed','up_to','by','average','percent','remainder','schedule'
    pub kind: String,
    /// JSON object with variant params, e.g. `{"amount":7299}` for fixed,
    /// `{"cap":30000}` for up_to, `{"pct":0.5}` for percent.
    pub params_json: String,
    pub priority: i64,
    pub created_at: String,
}

/// Request body for `POST /api/rpc/create_funding_template`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct CreateFundingTemplateRequest {
    pub category_id: String,
    pub kind: String,
    pub params_json: Option<String>,
    pub priority: Option<i64>,
}

/// Request body for `POST /api/rpc/update_funding_template`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct UpdateFundingTemplateRequest {
    pub id: String,
    pub category_id: Option<String>,
    pub kind: Option<String>,
    pub params_json: Option<String>,
    pub priority: Option<i64>,
}

/// Request body for `POST /api/rpc/delete_funding_template`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct DeleteFundingTemplateRequest {
    pub id: String,
}

/// Request body for `POST /api/rpc/apply_templates`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct ApplyTemplatesRequest {
    pub month: String,
}

/// One budget change produced by `apply_templates`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[schema(rename_all = "camelCase")]
pub struct BudgetChange {
    pub category_id: String,
    pub amount_cents: i64,
}
