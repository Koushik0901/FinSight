use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

/// A first-class record of an automated categorization SUGGESTION for a
/// transaction, decoupled from the canonical `transactions.category_id`
/// write it may or may not have made (see `applied`).
///
/// One LIVE row per transaction (`txn_id` is UNIQUE in the schema): a new
/// automated suggestion for the same transaction supersedes whatever
/// proposal was there before, so `status == "pending"` always means "the
/// current outstanding suggestion". Full per-attempt provenance already
/// lives in the append-only `categorizations` table (V003) — this table
/// exists to drive the review queue, not to be a second audit log.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryProposal {
    pub id: String,
    pub txn_id: String,
    pub proposed_category_id: String,
    /// 'llm' today; reserved for a future ML pass ('ml') per issue #87 scope —
    /// that pass is not built in this issue.
    pub source: String,
    pub confidence: f64,
    pub rationale: Option<String>,
    /// Ranked candidate categories as a JSON array string (opaque to
    /// finsight-core — not parsed or validated here). NULL until a
    /// multi-candidate pass exists; today it holds the single winning
    /// candidate for forward compatibility.
    pub candidates_json: Option<String>,
    /// "pending" | "accepted" | "corrected" | "rejected"
    pub status: String,
    /// Whether THIS proposal's category was written to
    /// `transactions.category_id` when the row was created. Always `true`
    /// today (the LLM pass still auto-writes canonical exactly as before);
    /// a future ML pass could insert `applied = false` (a suggestion with no
    /// canonical write) — the schema can express that without another
    /// migration, even though this issue does not build the unapplied path.
    pub applied: bool,
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
    /// Set when a human resolves the proposal via accept/correct/reject.
    /// NULL means either still pending, or auto-accepted without review
    /// (status = "accepted" with reviewed_at = NULL is the auto case; a
    /// human decision always stamps this).
    pub reviewed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewCategoryProposal {
    pub txn_id: String,
    pub proposed_category_id: String,
    pub source: String,
    pub confidence: f64,
    pub rationale: Option<String>,
    pub candidates_json: Option<String>,
    /// Caller-computed: "pending" if below the caller's confidence
    /// threshold, else "accepted" (auto-accepted because confidence was
    /// high — distinguished from a human acceptance by `reviewed_at` staying
    /// NULL). finsight-core does not own the threshold constant (it lives in
    /// finsight-agent, which depends on finsight-core, not the reverse), so
    /// the status decision is made by the caller.
    pub status: String,
    pub applied: bool,
    pub model: Option<String>,
}
