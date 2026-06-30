use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    pub id: String,
    pub source: String,
    pub import_id: Option<String>,
    pub sync_run_id: Option<String>,
    pub account_id: String,
    pub candidate_json: String,
    pub raw_payload_json: Option<String>,
    pub imported_id: Option<String>,
    pub external_tx_id: Option<String>,
    pub external_account_id: Option<String>,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub confidence: i64,
    pub reason: String,
    pub status: String,
    pub resolution: Option<String>,
    pub resolved_transaction_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewImportCandidate {
    pub source: String,
    pub import_id: Option<String>,
    pub sync_run_id: Option<String>,
    pub account_id: String,
    pub candidate_json: String,
    pub raw_payload_json: Option<String>,
    pub imported_id: Option<String>,
    pub external_tx_id: Option<String>,
    pub external_account_id: Option<String>,
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub confidence: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateMatch {
    pub id: String,
    pub candidate_id: String,
    pub transaction_id: String,
    pub match_kind: String,
    pub score: i64,
    pub is_recommended: bool,
    pub explanation_json: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct NewImportCandidateMatch {
    pub transaction_id: String,
    pub match_kind: String,
    pub score: i64,
    pub is_recommended: bool,
    pub explanation_json: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidateWithMatches {
    pub candidate: ImportCandidate,
    pub matches: Vec<ImportCandidateMatch>,
}
