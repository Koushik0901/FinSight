use crate::error::CoreResult;
use crate::models::{CategoryProposal, NewCategoryProposal};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

fn parse_dt(s: &str, col: usize) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(col, rusqlite::types::Type::Text, Box::new(e))
        })
}

const SELECT_COLUMNS: &str = "id, txn_id, proposed_category_id, source, confidence, rationale, \
     candidates_json, status, applied, model, created_at, reviewed_at";

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<CategoryProposal> {
    let created_at_s: String = r.get(10)?;
    let reviewed_at_s: Option<String> = r.get(11)?;
    Ok(CategoryProposal {
        id: r.get(0)?,
        txn_id: r.get(1)?,
        proposed_category_id: r.get(2)?,
        source: r.get(3)?,
        confidence: r.get(4)?,
        rationale: r.get(5)?,
        candidates_json: r.get(6)?,
        status: r.get(7)?,
        applied: r.get::<_, i64>(8)? != 0,
        model: r.get(9)?,
        created_at: parse_dt(&created_at_s, 10)?,
        reviewed_at: reviewed_at_s.map(|s| parse_dt(&s, 11)).transpose()?,
    })
}

/// Create the current outstanding proposal for a transaction, superseding
/// whatever *still-pending* proposal row already existed for it (one live row
/// per `txn_id` — see the migration comment for why). `reviewed_at` is reset
/// to NULL on supersede: a freshly (re)proposed suggestion has not been
/// reviewed yet.
///
/// ## Supersede policy: a RESOLVED proposal is never re-opened
///
/// The `ON CONFLICT` branch is guarded on `status = 'pending'`. Once a human
/// has accepted / corrected / rejected a proposal, an automated re-proposal
/// leaves the row completely untouched — the same category or a different
/// one, it makes no difference. The call still succeeds and returns the
/// (unchanged) resolved row, so re-proposal is a silent no-op for callers.
///
/// Why uniformly, rather than "re-open only if the category differs":
/// `status` + `reviewed_at` are the ONLY durable record of the human's
/// decision (there is one live row per `txn_id` and no proposal history
/// table — `categorizations` logs canonical writes, not suggestions). A
/// re-proposal carries no new information about that decision; a *differing*
/// category is model sampling noise, not evidence the user was wrong. If a
/// differing category could re-open the row, a rejection would be trivially
/// defeatable — the model need only guess differently on the next re-check
/// and the "rejected" verdict is erased with no trace.
///
/// The honest tradeoff: a resolved proposal now permanently blocks any future
/// automated proposal for that transaction. That is the safe direction (it
/// can never erase a decision the user made), and loosening it — e.g. a
/// "re-open after N days" or an explicit user-initiated re-propose — is a
/// separate, additive change.
///
/// Note that `reject` deliberately does not null `transactions.ai_confidence`,
/// so a rejected row keeps matching the categorizer's `load_low_confidence`
/// query and gets re-proposed-then-suppressed on every "Re-check". That is
/// harmless churn (the review queue reads `status = 'pending'`, which this
/// guard keeps it out of) — it must NOT be "fixed" by having reject mutate
/// canonical AI columns.
pub fn upsert(conn: &mut Connection, row: NewCategoryProposal) -> CoreResult<CategoryProposal> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO category_proposals \
            (id, txn_id, proposed_category_id, source, confidence, rationale, candidates_json, status, applied, model, created_at, reviewed_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL) \
         ON CONFLICT(txn_id) DO UPDATE SET \
            id = excluded.id, \
            proposed_category_id = excluded.proposed_category_id, \
            source = excluded.source, \
            confidence = excluded.confidence, \
            rationale = excluded.rationale, \
            candidates_json = excluded.candidates_json, \
            status = excluded.status, \
            applied = excluded.applied, \
            model = excluded.model, \
            created_at = excluded.created_at, \
            reviewed_at = NULL \
         WHERE category_proposals.status = 'pending'",
        params![
            id,
            row.txn_id,
            row.proposed_category_id,
            row.source,
            row.confidence,
            row.rationale,
            row.candidates_json,
            row.status,
            row.applied,
            row.model,
            now,
        ],
    )?;
    get_for_txn(conn, &row.txn_id)?.ok_or_else(|| {
        crate::error::CoreError::InvalidState(
            "category_proposals upsert did not round-trip".to_string(),
        )
    })
}

pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<CategoryProposal>> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM category_proposals WHERE id = ?1"),
        params![id],
        map_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn get_for_txn(conn: &mut Connection, txn_id: &str) -> CoreResult<Option<CategoryProposal>> {
    conn.query_row(
        &format!("SELECT {SELECT_COLUMNS} FROM category_proposals WHERE txn_id = ?1"),
        params![txn_id],
        map_row,
    )
    .optional()
    .map_err(Into::into)
}

pub fn list(conn: &mut Connection, status: Option<&str>) -> CoreResult<Vec<CategoryProposal>> {
    let mut out = Vec::new();
    match status {
        Some(s) => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM category_proposals WHERE status = ?1 ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map(params![s], map_row)?;
            for row in rows {
                out.push(row?);
            }
        }
        None => {
            let mut stmt = conn.prepare(&format!(
                "SELECT {SELECT_COLUMNS} FROM category_proposals ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map([], map_row)?;
            for row in rows {
                out.push(row?);
            }
        }
    }
    Ok(out)
}

pub fn count(conn: &mut Connection, status: &str) -> CoreResult<i64> {
    conn.query_row(
        "SELECT COUNT(*) FROM category_proposals WHERE status = ?1",
        params![status],
        |r| r.get(0),
    )
    .map_err(Into::into)
}

/// Resolve a specific proposal by id — the accept/correct/reject entry
/// point. Stamps `reviewed_at`, marking this as a human decision (as opposed
/// to the auto-accepted case, where `reviewed_at` stays NULL).
pub fn set_status(conn: &mut Connection, id: &str, status: &str) -> CoreResult<()> {
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "UPDATE category_proposals SET status = ?1, reviewed_at = ?2 WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

/// Resolve the pending proposal (if any) for a transaction whose canonical
/// category is being written through a path OTHER than accept/correct/reject
/// — chiefly the ordinary transaction-edit drawer, which calls
/// `repos::transactions::update` directly. Without this hook a proposal
/// left "pending" here would linger in the review queue forever after a
/// user manually recategorizes the transaction some other way, even though
/// today's `ai_confidence`-based predicate would have dropped it (`update`
/// always resets `ai_confidence` to NULL). Called from `transactions::update`
/// itself so every category-writing path stays in sync — see acceptance
/// criterion #2 on issue #87.
///
/// `new_category_id` is `None` when the category was explicitly cleared
/// (counts as "rejected" — the user disagreed and supplied no replacement).
pub fn resolve_for_txn(
    conn: &mut Connection,
    txn_id: &str,
    new_category_id: Option<&str>,
) -> CoreResult<()> {
    if let Some(existing) = get_for_txn(conn, txn_id)? {
        if existing.status == "pending" {
            let new_status = match new_category_id {
                Some(c) if c == existing.proposed_category_id => "accepted",
                Some(_) => "corrected",
                None => "rejected",
            };
            set_status(conn, &existing.id, new_status)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let (dir, db) = crate::testing::migrated_db();
        (dir, db)
    }

    fn seed_txn(conn: &mut Connection, txn_id: &str, cat_id: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES(?1,'a1','2024-01-01T00:00:00Z',-1000,'AMAZON','cleared',0,'2024-01-01T00:00:00Z')",
            params![txn_id],
        ).unwrap();
        let _ = cat_id;
    }

    fn seed_base(conn: &mut Connection) {
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat2','g1','Shopping','#0f0',1)", []).unwrap();
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')", []).unwrap();
    }

    #[test]
    fn insert_and_get_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");

        let created = upsert(
            &mut conn,
            NewCategoryProposal {
                txn_id: "t1".to_string(),
                proposed_category_id: "cat1".to_string(),
                source: "llm".to_string(),
                confidence: 0.42,
                rationale: Some("looked like food".to_string()),
                candidates_json: Some(r#"[{"category_id":"cat1","confidence":0.42}]"#.to_string()),
                status: "pending".to_string(),
                applied: true,
                model: Some("test-model".to_string()),
            },
        )
        .unwrap();

        assert_eq!(created.txn_id, "t1");
        assert_eq!(created.status, "pending");
        assert!(created.reviewed_at.is_none());

        let fetched = get(&mut conn, &created.id).unwrap().unwrap();
        assert_eq!(fetched.proposed_category_id, "cat1");
        assert!((fetched.confidence - 0.42).abs() < 1e-9);
        assert_eq!(fetched.model.as_deref(), Some("test-model"));

        let via_txn = get_for_txn(&mut conn, "t1").unwrap().unwrap();
        assert_eq!(via_txn.id, created.id);
    }

    #[test]
    fn upsert_supersedes_prior_pending_proposal_for_the_same_txn() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");

        let first = upsert(
            &mut conn,
            NewCategoryProposal {
                txn_id: "t1".to_string(),
                proposed_category_id: "cat1".to_string(),
                source: "llm".to_string(),
                confidence: 0.3,
                rationale: None,
                candidates_json: None,
                status: "pending".to_string(),
                applied: true,
                model: None,
            },
        )
        .unwrap();

        let second = upsert(
            &mut conn,
            NewCategoryProposal {
                txn_id: "t1".to_string(),
                proposed_category_id: "cat2".to_string(),
                source: "llm".to_string(),
                confidence: 0.9,
                rationale: None,
                candidates_json: None,
                status: "accepted".to_string(),
                applied: true,
                model: None,
            },
        )
        .unwrap();

        assert_ne!(first.id, second.id, "a fresh suggestion gets a fresh id");
        assert!(get(&mut conn, &first.id).unwrap().is_none(), "superseded row is gone, not orphaned");
        let live = get_for_txn(&mut conn, "t1").unwrap().unwrap();
        assert_eq!(live.proposed_category_id, "cat2");
        assert_eq!(live.status, "accepted");
        assert_eq!(count(&mut conn, "pending").unwrap(), 0);
    }

    /// Regression (review finding 1a): a RESOLVED proposal must never be
    /// resurrected into `pending` by an automated re-proposal.
    ///
    /// The concrete scenario: the user rejects an LLM "Food" guess on t1.
    /// `reject` deliberately leaves `transactions.ai_confidence` alone, so t1
    /// still matches the categorizer's `load_low_confidence` query and gets
    /// re-sent to the LLM on the next "Re-check" — which the Inbox action
    /// item's own copy invites. If `upsert` superseded unconditionally, the
    /// rejection would be silently erased (status back to `pending`,
    /// `reviewed_at` back to NULL) with no trace, and Accept would then write
    /// the rejected guess to canonical.
    #[test]
    fn upsert_does_not_resurrect_a_resolved_proposal() {
        for (resolved_status, reproposed_category) in [
            // The model repeats itself…
            ("rejected", "cat1"),
            // …or guesses differently. Neither may re-open the row: a
            // differing category is sampling noise, not evidence the user's
            // decision was wrong — see the `upsert` supersede policy.
            ("rejected", "cat2"),
            ("accepted", "cat2"),
            ("corrected", "cat2"),
        ] {
            let (_d, db) = fresh_db();
            let mut conn = db.get().unwrap();
            seed_base(&mut conn);
            seed_txn(&mut conn, "t1", "cat1");

            let p = upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: "cat1".to_string(),
                    source: "llm".to_string(),
                    confidence: 0.3,
                    rationale: None,
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();
            set_status(&mut conn, &p.id, resolved_status).unwrap();
            let resolved = get(&mut conn, &p.id).unwrap().unwrap();

            // The re-check re-proposes for the same transaction.
            let returned = upsert(
                &mut conn,
                NewCategoryProposal {
                    txn_id: "t1".to_string(),
                    proposed_category_id: reproposed_category.to_string(),
                    source: "llm".to_string(),
                    confidence: 0.45,
                    rationale: Some("re-check".to_string()),
                    candidates_json: None,
                    status: "pending".to_string(),
                    applied: true,
                    model: None,
                },
            )
            .unwrap();

            // Row is untouched — including `reviewed_at`, which is what proves
            // the human decision itself was not re-stamped or cleared.
            let after = get_for_txn(&mut conn, "t1").unwrap().unwrap();
            assert_eq!(after.id, p.id, "{resolved_status}: no fresh row replaced it");
            assert_eq!(after.status, resolved_status, "{resolved_status}: status preserved");
            assert_eq!(
                after.proposed_category_id, "cat1",
                "{resolved_status}: the resolved row's category is not overwritten"
            );
            assert_eq!(
                after.reviewed_at, resolved.reviewed_at,
                "{resolved_status}: the human decision timestamp is untouched"
            );
            assert!(
                (after.confidence - 0.3).abs() < 1e-9,
                "{resolved_status}: the re-proposal's confidence did not leak in"
            );
            // The suppressed call still succeeds and reports the live row.
            assert_eq!(returned.id, p.id);
            assert_eq!(returned.status, resolved_status);
            // And critically: it never re-enters the review queue.
            assert_eq!(count(&mut conn, "pending").unwrap(), 0, "{resolved_status}");
        }
    }

    #[test]
    fn list_filters_by_status() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        seed_txn(&mut conn, "t2", "cat1");

        upsert(&mut conn, NewCategoryProposal {
            txn_id: "t1".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.3, rationale: None,
            candidates_json: None, status: "pending".to_string(), applied: true, model: None,
        }).unwrap();
        upsert(&mut conn, NewCategoryProposal {
            txn_id: "t2".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.95, rationale: None,
            candidates_json: None, status: "accepted".to_string(), applied: true, model: None,
        }).unwrap();

        let pending = list(&mut conn, Some("pending")).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].txn_id, "t1");
        assert_eq!(count(&mut conn, "pending").unwrap(), 1);
        assert_eq!(count(&mut conn, "accepted").unwrap(), 1);
    }

    #[test]
    fn set_status_stamps_reviewed_at() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        let p = upsert(&mut conn, NewCategoryProposal {
            txn_id: "t1".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.3, rationale: None,
            candidates_json: None, status: "pending".to_string(), applied: true, model: None,
        }).unwrap();
        assert!(p.reviewed_at.is_none());

        set_status(&mut conn, &p.id, "rejected").unwrap();
        let after = get(&mut conn, &p.id).unwrap().unwrap();
        assert_eq!(after.status, "rejected");
        assert!(after.reviewed_at.is_some());
    }

    #[test]
    fn resolve_for_txn_marks_accepted_when_category_matches() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        let p = upsert(&mut conn, NewCategoryProposal {
            txn_id: "t1".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.3, rationale: None,
            candidates_json: None, status: "pending".to_string(), applied: true, model: None,
        }).unwrap();

        resolve_for_txn(&mut conn, "t1", Some("cat1")).unwrap();
        let after = get(&mut conn, &p.id).unwrap().unwrap();
        assert_eq!(after.status, "accepted");
        assert!(after.reviewed_at.is_some());
    }

    #[test]
    fn resolve_for_txn_marks_corrected_when_category_differs() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        let p = upsert(&mut conn, NewCategoryProposal {
            txn_id: "t1".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.3, rationale: None,
            candidates_json: None, status: "pending".to_string(), applied: true, model: None,
        }).unwrap();

        resolve_for_txn(&mut conn, "t1", Some("cat2")).unwrap();
        let after = get(&mut conn, &p.id).unwrap().unwrap();
        assert_eq!(after.status, "corrected");
    }

    #[test]
    fn resolve_for_txn_marks_rejected_when_category_cleared() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        let p = upsert(&mut conn, NewCategoryProposal {
            txn_id: "t1".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.3, rationale: None,
            candidates_json: None, status: "pending".to_string(), applied: true, model: None,
        }).unwrap();

        resolve_for_txn(&mut conn, "t1", None).unwrap();
        let after = get(&mut conn, &p.id).unwrap().unwrap();
        assert_eq!(after.status, "rejected");
    }

    #[test]
    fn resolve_for_txn_is_a_no_op_when_no_proposal_exists() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        // No proposal ever created for t1 — must not error or fabricate a row.
        resolve_for_txn(&mut conn, "t1", Some("cat1")).unwrap();
        assert!(get_for_txn(&mut conn, "t1").unwrap().is_none());
    }

    #[test]
    fn resolve_for_txn_leaves_an_already_resolved_proposal_alone() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_base(&mut conn);
        seed_txn(&mut conn, "t1", "cat1");
        let p = upsert(&mut conn, NewCategoryProposal {
            txn_id: "t1".to_string(), proposed_category_id: "cat1".to_string(),
            source: "llm".to_string(), confidence: 0.3, rationale: None,
            candidates_json: None, status: "pending".to_string(), applied: true, model: None,
        }).unwrap();
        set_status(&mut conn, &p.id, "rejected").unwrap();
        let rejected_at = get(&mut conn, &p.id).unwrap().unwrap().reviewed_at;

        // A later, unrelated category write on the same txn must not
        // resurrect or re-stamp an already-resolved proposal.
        resolve_for_txn(&mut conn, "t1", Some("cat2")).unwrap();
        let after = get(&mut conn, &p.id).unwrap().unwrap();
        assert_eq!(after.status, "rejected");
        assert_eq!(after.reviewed_at, rejected_at);
    }
}
