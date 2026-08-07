//! Issue #87 (Slice 1): review-queue commands for `category_proposals`.
//!
//! `accept`/`correct` both delegate to `repos::transactions::update` for the
//! canonical write — that reproduces the exact synchronous effects a manual
//! edit-transaction call already has (write `category_id`, append a
//! `source='user'` `categorizations` audit row, call
//! `agent_memory::upsert_correction`, and surface the same inline
//! `ProposedRule`) instead of re-implementing them. Neither calls
//! `rule_proposals::emit_from_corrections` — that stays exclusively an async
//! categorizer-job step gated on >=3 accumulated `source='user'` corrections;
//! the audit row this appends is picked up automatically on the categorizer's
//! next run. `reject` does NOT touch canonical at all: it only dismisses the
//! proposal from the review queue.

use crate::commands::transactions::{ProposedRuleDto, UpdateTxnResult};
use crate::error::{AppError, AppResult};
use crate::ApiState;
use finsight_core::models::{CategoryProposal, TxnPatch};
use finsight_core::repos::{category_proposals, run, transactions};
use finsight_core::CoreError;
use rusqlite::OptionalExtension;

/// The current review queue — proposals still awaiting a human decision.
pub async fn list_category_proposals(state: &ApiState) -> AppResult<Vec<CategoryProposal>> {
    let db = (*state.db).clone();
    run(&db, |conn| category_proposals::list(conn, Some("pending")))
        .await
        .map_err(AppError::from)
}

fn pending_or_error(
    conn: &mut rusqlite::Connection,
    id: &str,
) -> Result<CategoryProposal, CoreError> {
    let proposal = category_proposals::get(conn, id)?
        .ok_or_else(|| CoreError::InvalidState(format!("category proposal `{id}` not found")))?;
    if proposal.status != "pending" {
        return Err(CoreError::InvalidState(format!(
            "category proposal `{id}` was already resolved (`{}`)",
            proposal.status
        )));
    }
    Ok(proposal)
}

/// The category being written must still exist AND be active. The FK on
/// `transactions.category_id` only catches ids that do not exist — an
/// ARCHIVED category still has a row, so the FK happily accepts it. A
/// proposal can easily outlive its target: the LLM proposes category X, the
/// user archives X while consolidating their category list, then clicks
/// Accept. Without this check that money silently drops out of every
/// active-category view. Mirrors the precedent in
/// `finsight_agent::executor`'s `recategorize_bulk` arm, which runs this
/// exact query before applying an assignment.
fn active_category_or_error(
    conn: &mut rusqlite::Connection,
    category_id: &str,
) -> Result<(), CoreError> {
    let active: bool = conn
        .query_row(
            "SELECT 1 FROM categories WHERE id = ?1 AND archived_at IS NULL",
            [category_id],
            |_| Ok(true),
        )
        .optional()?
        .unwrap_or(false);
    if !active {
        return Err(CoreError::InvalidState(format!(
            "category `{category_id}` no longer exists or has been archived"
        )));
    }
    Ok(())
}

/// The user agrees with the proposed category. Writes it as if the user had
/// typed it into the transaction-edit drawer — the write is a `source='user'`
/// categorization from this point on, not an unreviewed AI guess.
pub async fn accept_category_proposal(state: &ApiState, id: String) -> AppResult<UpdateTxnResult> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let proposal = pending_or_error(conn, &id)?;
        active_category_or_error(conn, &proposal.proposed_category_id)?;
        let (transaction, rule) = transactions::update(
            conn,
            &proposal.txn_id,
            TxnPatch {
                category_id: Some(Some(proposal.proposed_category_id.clone())),
                ..Default::default()
            },
        )?;
        Ok(UpdateTxnResult {
            transaction,
            proposed_rule: rule.map(|r| ProposedRuleDto {
                pattern: r.pattern,
                category_id: r.category_id,
                category_label: r.category_label,
            }),
        })
    })
    .await
    .map_err(AppError::from)
}

/// The user picks a DIFFERENT category than what was proposed.
pub async fn correct_category_proposal(
    state: &ApiState,
    id: String,
    category_id: String,
) -> AppResult<UpdateTxnResult> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let proposal = pending_or_error(conn, &id)?;
        active_category_or_error(conn, &category_id)?;
        let (transaction, rule) = transactions::update(
            conn,
            &proposal.txn_id,
            TxnPatch {
                category_id: Some(Some(category_id)),
                ..Default::default()
            },
        )?;
        Ok(UpdateTxnResult {
            transaction,
            proposed_rule: rule.map(|r| ProposedRuleDto {
                pattern: r.pattern,
                category_id: r.category_id,
                category_label: r.category_label,
            }),
        })
    })
    .await
    .map_err(AppError::from)
}

/// The user dismisses the proposal without providing a replacement category.
/// Canonical `transactions.category_id` is left untouched (it already holds
/// the LLM's applied guess) — this only removes the item from the review
/// queue.
pub async fn reject_category_proposal(state: &ApiState, id: String) -> AppResult<()> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        pending_or_error(conn, &id)?;
        category_proposals::set_status(conn, &id, "rejected")
    })
    .await
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::models::NewCategoryProposal;
    use finsight_core::repos::rule_proposals;

    use std::sync::Arc;
    use tempfile::TempDir;

    fn fresh_state() -> (TempDir, ApiState) {
        let (dir, db) = finsight_core::testing::migrated_db();
        let state = ApiState::new(db, dir.path().to_path_buf(), Arc::new(|_| {}));
        (dir, state)
    }

    fn seed_base(conn: &mut rusqlite::Connection) {
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Coffee','#f00',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat2','g1','Shopping','#0f0',1)", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
    }

    fn seed_txn(conn: &mut rusqlite::Connection, id: &str, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES(?1,'a1','2024-01-15T00:00:00Z',-500,?2,'cleared',0,'2024-01-15T00:00:00Z')",
            rusqlite::params![id, merchant],
        ).unwrap();
    }

    /// Acceptance criterion #1: accept must reproduce `transactions::update`'s
    /// synchronous effects (write category_id, append a source='user'
    /// categorizations row, run the agent_memory + inline-ProposedRule logic)
    /// by literally delegating to it — and must NOT itself trigger
    /// `rule_proposals::emit_from_corrections`, even when the merchant already
    /// has >=3 accumulated user corrections (the condition that WOULD make
    /// the categorizer's async post-run step emit one).
    #[tokio::test]
    async fn accept_does_not_emit_a_rule_proposal_even_past_the_correction_threshold() {
        let (_dir, state) = fresh_state();
        {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
            // Three prior user corrections for BEANS CAFE -> cat1: exactly the
            // condition `rule_proposals::emit_from_corrections(_, 3)` looks for.
            for i in 0..3 {
                let tid = format!("prior{i}");
                seed_txn(&mut conn, &tid, "BEANS CAFE");
                conn.execute(
                    "UPDATE transactions SET category_id='cat1' WHERE id=?1",
                    rusqlite::params![tid],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO categorizations(id,txn_id,category_id,source,confidence,at) \
                     VALUES(?1,?2,'cat1','user',1.0,'2024-01-16T00:00:00Z')",
                    rusqlite::params![format!("c{i}"), tid],
                )
                .unwrap();
            }
            // A 4th BEANS CAFE transaction the LLM proposed at low confidence.
            seed_txn(&mut conn, "t4", "BEANS CAFE");
            category_proposals::upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t4".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.4,
                    rationale: Some("looks like a cafe".to_string()),
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: Some("test-model".to_string()),
                },
            )
            .unwrap();
        }

        let proposal_id = {
            let mut conn = state.db.get().unwrap();
            category_proposals::get_for_txn(&mut conn, "t4")
                .unwrap()
                .unwrap()
                .id
        };

        let result = accept_category_proposal(&state, proposal_id.clone())
            .await
            .expect("accept should succeed");
        assert_eq!(result.transaction.category_id.as_deref(), Some("cat1"));

        let mut conn = state.db.get().unwrap();
        // Same synchronous effects `update` always has:
        let user_rows: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM categorizations WHERE txn_id='t4' AND source='user'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            user_rows, 1,
            "accept appends a source='user' categorizations row"
        );

        // The proposal itself is resolved as a human decision.
        let resolved = category_proposals::get(&mut conn, &proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.status, "accepted");
        assert!(resolved.reviewed_at.is_some());

        // THE assertion: even though the merchant now has 4 user corrections
        // (past the threshold of 3), accept must not have synchronously
        // fired `emit_from_corrections` — only the categorizer's async
        // post-run step does that.
        let pending_rule_proposals = rule_proposals::list(&mut conn, Some("pending")).unwrap();
        assert!(
            pending_rule_proposals.is_empty(),
            "accept must not directly invoke rule_proposals::emit_from_corrections"
        );

        // Sanity: running the emission pass explicitly (what the categorizer
        // job does on its own schedule) DOES pick it up — proving the audit
        // row accept left behind is real, discoverable input for it.
        let emitted = rule_proposals::emit_from_corrections(&mut conn, 3).unwrap();
        assert_eq!(emitted, 1);
    }

    #[tokio::test]
    async fn correct_writes_the_chosen_category_not_the_proposed_one() {
        let (_dir, state) = fresh_state();
        let proposal_id = {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
            seed_txn(&mut conn, "t1", "AMBIGUOUS STORE");
            let p = category_proposals::upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.4,
                    rationale: None,
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
            p.id
        };

        let result = correct_category_proposal(&state, proposal_id.clone(), "cat2".to_string())
            .await
            .unwrap();
        assert_eq!(result.transaction.category_id.as_deref(), Some("cat2"));

        let mut conn = state.db.get().unwrap();
        let resolved = category_proposals::get(&mut conn, &proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.status, "corrected");
        assert!(resolved.reviewed_at.is_some());
    }

    #[tokio::test]
    async fn reject_leaves_canonical_category_untouched() {
        let (_dir, state) = fresh_state();
        let proposal_id = {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
            seed_txn(&mut conn, "t1", "MYSTERY MERCHANT");
            // The LLM pass already applied its guess (additive slice: canonical
            // is written regardless of confidence).
            conn.execute(
                "UPDATE transactions SET category_id='cat1', ai_confidence=0.4 WHERE id='t1'",
                [],
            )
            .unwrap();
            let p = category_proposals::upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.4,
                    rationale: None,
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
            p.id
        };

        reject_category_proposal(&state, proposal_id.clone())
            .await
            .unwrap();

        let mut conn = state.db.get().unwrap();
        let cat_id: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cat_id.as_deref(),
            Some("cat1"),
            "reject must not clear the canonical category the LLM already applied"
        );
        let resolved = category_proposals::get(&mut conn, &proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(resolved.status, "rejected");

        // No longer counted in the review queue.
        assert_eq!(category_proposals::count(&mut conn, "pending").unwrap(), 0);
    }

    #[tokio::test]
    async fn accept_on_an_already_resolved_proposal_errors_instead_of_reapplying() {
        let (_dir, state) = fresh_state();
        let proposal_id = {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
            seed_txn(&mut conn, "t1", "STORE");
            let p = category_proposals::upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.4,
                    rationale: None,
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
            p.id
        };

        reject_category_proposal(&state, proposal_id.clone())
            .await
            .unwrap();
        let second = accept_category_proposal(&state, proposal_id).await;
        assert!(
            second.is_err(),
            "accepting an already-rejected proposal must error, not silently reapply"
        );
    }

    /// Regression (review finding 3): a proposal can outlive its target
    /// category. The FK on `transactions.category_id` only rejects ids that do
    /// not EXIST — an archived category still has a row, so nothing else stops
    /// accept/correct from writing it. The harm is silent: the transaction
    /// keeps a category that every active-category view filters out, so the
    /// money simply vanishes from budgets and reports.
    #[tokio::test]
    async fn accept_onto_an_archived_category_errors_and_leaves_canonical_untouched() {
        let (_dir, state) = fresh_state();
        let proposal_id = {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
            seed_txn(&mut conn, "t1", "BEANS CAFE");
            let p = category_proposals::upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.4,
                    rationale: None,
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
            // The user consolidates their category list AFTER the proposal
            // was made.
            conn.execute(
                "UPDATE categories SET archived_at = '2024-02-01T00:00:00Z' WHERE id = 'cat1'",
                [],
            )
            .unwrap();
            p.id
        };

        let result = accept_category_proposal(&state, proposal_id.clone()).await;
        assert!(result.is_err(), "accept must refuse an archived category");

        let mut conn = state.db.get().unwrap();
        let cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cat, None,
            "no archived category may be written to canonical"
        );
        // The proposal is still actionable — the user can correct it to a live
        // category instead of being stuck with an un-resolvable queue item.
        let still = category_proposals::get(&mut conn, &proposal_id)
            .unwrap()
            .unwrap();
        assert_eq!(still.status, "pending");
    }

    #[tokio::test]
    async fn correct_onto_an_archived_category_errors_and_leaves_canonical_untouched() {
        let (_dir, state) = fresh_state();
        let proposal_id = {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
            seed_txn(&mut conn, "t1", "BEANS CAFE");
            let p = category_proposals::upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.4,
                    rationale: None,
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
            conn.execute(
                "UPDATE categories SET archived_at = '2024-02-01T00:00:00Z' WHERE id = 'cat2'",
                [],
            )
            .unwrap();
            p.id
        };

        // User picks cat2 — which they archived.
        let result =
            correct_category_proposal(&state, proposal_id.clone(), "cat2".to_string()).await;
        assert!(result.is_err(), "correct must refuse an archived category");

        let mut conn = state.db.get().unwrap();
        let cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            cat, None,
            "no archived category may be written to canonical"
        );
        assert_eq!(
            category_proposals::get(&mut conn, &proposal_id)
                .unwrap()
                .unwrap()
                .status,
            "pending"
        );

        // Correcting to a still-active category works normally.
        correct_category_proposal(&state, proposal_id.clone(), "cat1".to_string())
            .await
            .expect("an active category is still accepted");
        let cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cat.as_deref(), Some("cat1"));
    }

    #[tokio::test]
    async fn accept_on_unknown_id_errors() {
        let (_dir, state) = fresh_state();
        {
            let mut conn = state.db.get().unwrap();
            seed_base(&mut conn);
        }
        let result = accept_category_proposal(&state, "does-not-exist".to_string()).await;
        assert!(result.is_err());
    }
}
