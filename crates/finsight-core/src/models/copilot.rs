use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentSession {
    pub id: String,
    pub title: String,
    pub status: String,
    pub task_type: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionBundle {
    pub id: String,
    pub session_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub rationale: String,
    pub confidence: f64,
    pub status: String,
    pub provider_id: Option<String>,
    pub model_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub items: Vec<AgentActionItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentActionItem {
    pub id: String,
    pub bundle_id: String,
    pub action_kind: String,
    pub payload_json: String,
    pub preview_json: Option<String>,
    pub rationale: String,
    pub confidence: f64,
    pub status: String,
    pub validation_errors: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Something the Copilot needed but could not find, and — where we can work it
/// out — where the user would go to supply it.
///
/// The Copilot deliberately withholds high-confidence debt advice when APR or
/// minimum-payment data is absent. That is the right call, but a block that
/// does not say how to unblock it reads as the app being unhelpful rather than
/// careful. Attaching a destination turns an honest limitation into a
/// completed setup step.
///
/// `action_label` and `action_path` are always set or cleared **together**: a
/// labelled button with nowhere to go, or a destination with no label, are
/// both worse than plain prose. Producers that do not know which entity is
/// missing data (notably the model itself, on the deep reasoning path) build
/// these through [`From<String>`], which leaves both `None` — the message
/// still renders, just without a shortcut.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MissingDataItem {
    /// Human-readable description of what is missing.
    pub message: String,
    /// Button text, e.g. "Add APR". `None` when there is nowhere to send them.
    pub action_label: Option<String>,
    /// App-relative path, e.g. `/accounts?focusAccount=abc`.
    pub action_path: Option<String>,
}

impl MissingDataItem {
    /// A message with no destination — the safe default.
    pub fn prose(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            action_label: None,
            action_path: None,
        }
    }

    /// A message the user can act on directly.
    ///
    /// Falls back to prose if either half is blank, so a caller can pass a
    /// best-effort label or path without risking a broken control.
    pub fn linked(
        message: impl Into<String>,
        action_label: impl Into<String>,
        action_path: impl Into<String>,
    ) -> Self {
        let label = action_label.into();
        let path = action_path.into();
        if label.trim().is_empty() || path.trim().is_empty() {
            return Self::prose(message);
        }
        Self {
            message: message.into(),
            action_label: Some(label),
            action_path: Some(path),
        }
    }

    /// Stable ordering/dedup key. Two items describing the same gap should
    /// collapse even if only one of them carried a destination.
    pub fn dedup_key(&self) -> &str {
        &self.message
    }

    /// Sort and de-duplicate a batch, keeping every distinct destination.
    ///
    /// Two things have to be true at once, and they pull in opposite
    /// directions:
    ///
    /// 1. The *same* gap often arrives from several producers — one that knows
    ///    which account it belongs to, and one that only has prose. Those must
    ///    collapse to a single actionable entry, otherwise the user sees the
    ///    same sentence twice and the CTA looks arbitrary.
    /// 2. *Different* entities can produce the same sentence. Account names
    ///    are user- and import-defined with no uniqueness constraint, so two
    ///    cards both called "Visa" both yield "Visa is missing APR." — with
    ///    different destinations. Collapsing those would silently strip one
    ///    account's link and leave the user unable to reach it.
    ///
    /// So the rule is: collapse by message, but keep one entry per distinct
    /// destination. Prose copies are absorbed into a linked copy of the same
    /// message when one exists, and survive alone when none does.
    ///
    /// Lives here rather than at each merge point so every path collapses
    /// duplicates the same way.
    pub fn dedup(items: &mut Vec<Self>) {
        items.sort_by(|a, b| {
            a.dedup_key()
                .cmp(b.dedup_key())
                // Actionable first within a message group, so a prose copy is
                // never the one that establishes the group's identity.
                .then_with(|| b.action_path.is_some().cmp(&a.action_path.is_some()))
                .then_with(|| a.action_path.cmp(&b.action_path))
        });
        let mut kept: Vec<Self> = Vec::with_capacity(items.len());
        for item in items.drain(..) {
            let duplicate = kept.iter().any(|seen| {
                seen.dedup_key() == item.dedup_key()
                    && match (&seen.action_path, &item.action_path) {
                        // Same message, same destination — the same gap twice.
                        (Some(a), Some(b)) => a == b,
                        // A prose copy of a gap we already have a link for.
                        (Some(_), None) => true,
                        // Sorting puts linked entries first, so a kept prose
                        // entry means no link exists for this message.
                        (None, _) => true,
                    }
            });
            if !duplicate {
                kept.push(item);
            }
        }
        *items = kept;
    }
}

impl From<String> for MissingDataItem {
    fn from(message: String) -> Self {
        Self::prose(message)
    }
}

impl From<&str> for MissingDataItem {
    fn from(message: &str) -> Self {
        Self::prose(message)
    }
}

/// A screen the user can jump to in order to *see* a change the agent made.
///
/// Always an offer rendered as a link — never an automatic redirect. Yanking
/// someone out of a conversation mid-thread is worse than making them click.
///
/// Built from executed action payloads (which carry real entity ids) rather
/// than from model-authored prose, so the destination is always a screen that
/// exists and an entity that was actually touched.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentNavigationTarget {
    /// Button text, e.g. "View in Budget".
    pub label: String,
    /// An app-relative path, e.g. `/budget?focusCategory=abc`.
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentExecutionEntry {
    pub id: String,
    pub item_id: String,
    pub bundle_id: String,
    pub action_kind: String,
    pub status: String,
    pub result_json: Option<String>,
    pub error: Option<String>,
    pub executed_at: String,
}

/// Summary of a conversation thread shown in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub message_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// A single message within a conversation thread.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: String,
    pub conversation_id: String,
    pub role: String,
    pub content: String,
    /// JSON-encoded array of tool names used, e.g. `["spending_by_category"]`
    pub tool_trace: Option<String>,
    pub action_bundle_id: Option<String>,
    pub branch_parent_id: Option<String>,
    /// JSON-encoded assistant-ui message parts. `content` remains the text fallback.
    pub parts_json: Option<String>,
    /// Run lifecycle state for AG-UI/assistant reload semantics.
    pub run_status: String,
    /// JSON-encoded AG-UI metadata for tool calls, artifacts, approvals, and usage.
    pub ag_ui_metadata_json: Option<String>,
    pub created_at: String,
}

#[cfg(test)]
mod missing_data_tests {
    use super::MissingDataItem;

    #[test]
    fn prose_items_carry_no_call_to_action() {
        let item = MissingDataItem::prose("Add APRs before finalizing.");
        assert_eq!(item.message, "Add APRs before finalizing.");
        assert!(item.action_label.is_none());
        assert!(item.action_path.is_none());
    }

    #[test]
    fn model_authored_strings_degrade_to_prose() {
        // The deep reasoning path lets the model author these, so it has no
        // entity to point at. It must still render, just without a shortcut.
        let item: MissingDataItem = "No APR on your card".to_string().into();
        assert!(item.action_path.is_none());
        let borrowed: MissingDataItem = "No minimum payment".into();
        assert!(borrowed.action_path.is_none());
    }

    #[test]
    fn linked_items_carry_both_halves() {
        let item = MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1");
        assert_eq!(item.action_label.as_deref(), Some("Add APR"));
        assert_eq!(item.action_path.as_deref(), Some("/accounts?focusAccount=a1"));
    }

    #[test]
    fn half_a_call_to_action_is_no_call_to_action() {
        // A labelled button with nowhere to go, or a destination with no
        // label, are both worse than plain prose.
        for (label, path) in [("", "/accounts"), ("Add APR", ""), ("  ", "  ")] {
            let item = MissingDataItem::linked("msg", label, path);
            assert!(item.action_label.is_none(), "label {label:?} path {path:?}");
            assert!(item.action_path.is_none(), "label {label:?} path {path:?}");
        }
    }

    #[test]
    fn dedup_keeps_the_actionable_copy_regardless_of_input_order() {
        // The same gap arrives from a producer that knows the account and one
        // that only has prose. Merging must not downgrade the CTA.
        let linked = MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1");
        let prose = MissingDataItem::prose("Visa is missing APR.");

        for input in [
            vec![prose.clone(), linked.clone()],
            vec![linked.clone(), prose.clone()],
        ] {
            let mut items = input;
            MissingDataItem::dedup(&mut items);
            assert_eq!(items.len(), 1);
            assert_eq!(items[0].action_path.as_deref(), Some("/accounts?focusAccount=a1"));
        }
    }

    #[test]
    fn dedup_keeps_genuinely_different_gaps() {
        let mut items = vec![
            MissingDataItem::prose("Visa is missing APR."),
            MissingDataItem::prose("Loan is missing minimum payment."),
            MissingDataItem::prose("Visa is missing APR."),
        ];
        MissingDataItem::dedup(&mut items);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn dedup_handles_empty_and_single_batches() {
        let mut empty: Vec<MissingDataItem> = Vec::new();
        MissingDataItem::dedup(&mut empty);
        assert!(empty.is_empty());

        let mut one = vec![MissingDataItem::prose("only")];
        MissingDataItem::dedup(&mut one);
        assert_eq!(one.len(), 1);
    }

    #[test]
    fn identically_named_accounts_each_keep_their_own_link() {
        // Account names are user- and import-defined with no uniqueness
        // constraint, so two cards can both be called "Visa" and produce the
        // same sentence with different destinations. Collapsing them would
        // strip one account's link and leave it unreachable.
        let mut items = vec![
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1"),
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a2"),
        ];
        MissingDataItem::dedup(&mut items);
        assert_eq!(items.len(), 2, "each account must remain reachable");

        let paths: Vec<_> = items.iter().filter_map(|i| i.action_path.as_deref()).collect();
        assert!(paths.contains(&"/accounts?focusAccount=a1"));
        assert!(paths.contains(&"/accounts?focusAccount=a2"));
    }

    #[test]
    fn a_prose_copy_is_absorbed_by_any_linked_copy_of_the_same_gap() {
        // Even when several links exist for one message, the prose duplicate
        // adds nothing and must not render as a fourth bullet.
        let mut items = vec![
            MissingDataItem::prose("Visa is missing APR."),
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1"),
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a2"),
            MissingDataItem::prose("Visa is missing APR."),
        ];
        MissingDataItem::dedup(&mut items);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|i| i.action_path.is_some()));
    }

    #[test]
    fn repeated_identical_links_still_collapse() {
        let mut items = vec![
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1"),
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1"),
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1"),
        ];
        MissingDataItem::dedup(&mut items);
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn two_accounts_missing_the_same_field_stay_separate() {
        // Distinct entities produce distinct messages, so both survive and
        // each keeps its own destination.
        let mut items = vec![
            MissingDataItem::linked("Visa is missing APR.", "Add APR", "/accounts?focusAccount=a1"),
            MissingDataItem::linked("Amex is missing APR.", "Add APR", "/accounts?focusAccount=a2"),
        ];
        MissingDataItem::dedup(&mut items);
        assert_eq!(items.len(), 2);
        let paths: Vec<_> = items.iter().filter_map(|i| i.action_path.as_deref()).collect();
        assert!(paths.contains(&"/accounts?focusAccount=a1"));
        assert!(paths.contains(&"/accounts?focusAccount=a2"));
    }
}
