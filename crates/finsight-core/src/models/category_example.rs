use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use specta::Type;

/// A user-curated exemplar transaction description for a category.
///
/// Keyed by the category's STABLE `id`, so examples survive a rename exactly
/// the way `categories.guidance` does (rename touches `label` only).
///
/// `example_text` is a denormalized snapshot rather than a join through
/// `source_txn_id`: a factory reset wipes `transactions`, and a CSV re-import
/// churns transaction ids, so a pure reference would silently lose a curated
/// exemplar set. `source_txn_id` is provenance only — NULL means hand-typed
/// or the source transaction is gone (the FK is `ON DELETE SET NULL`).
///
/// Nothing reads these yet. Issue #92 embeds `example_text` into a
/// prototype/centroid vector per category.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryExample {
    pub id: String,
    pub category_id: String,
    /// The exemplar description text (trimmed, never empty).
    pub example_text: String,
    /// Optional provenance breadcrumb back to the transaction the user added
    /// this from. NULL = hand-typed, or the transaction has since been deleted.
    pub source_txn_id: Option<String>,
    pub created_at: DateTime<Utc>,
}
