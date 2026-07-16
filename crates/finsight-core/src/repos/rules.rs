use crate::error::CoreResult;
use crate::models::{NewRule, Rule};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list_active(conn: &mut Connection) -> CoreResult<Vec<Rule>> {
    let mut stmt = conn.prepare(
        "SELECT id, pattern, category_id, enabled, source, created_at, treatment \
         FROM rules WHERE enabled = 1 ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        let created_s: String = r.get(5)?;
        Ok(Rule {
            id: r.get(0)?,
            pattern: r.get(1)?,
            category_id: r.get(2)?,
            enabled: r.get::<_, i64>(3)? != 0,
            source: r.get(4)?,
            created_at: DateTime::parse_from_rfc3339(&created_s)
                .map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        5,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?
                .with_timezone(&Utc),
            treatment: r.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

pub fn insert(conn: &mut Connection, rule: NewRule) -> CoreResult<Rule> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now();
    conn.execute(
        "INSERT INTO rules(id, pattern, category_id, enabled, source, created_at, treatment) \
         VALUES(?1, ?2, ?3, 1, ?4, ?5, ?6)",
        params![
            id,
            rule.pattern,
            rule.category_id,
            rule.source,
            now.to_rfc3339(),
            rule.treatment
        ],
    )?;
    Ok(Rule {
        id,
        pattern: rule.pattern,
        category_id: rule.category_id,
        enabled: true,
        source: rule.source,
        created_at: now,
        treatment: rule.treatment,
    })
}

/// Retroactively apply a rule pattern to existing UNCATEGORIZED, non-transfer
/// expense transactions, so a rule created from a recurring payment (e.g. an
/// e-transfer to a landlord → Housing) categorizes the history immediately
/// instead of only future imports. Returns the number of rows categorized.
/// Uses the same `%…%`=contains / bare=exact LIKE semantics as the categorizer.
pub fn apply_to_uncategorized(
    conn: &mut Connection,
    pattern: &str,
    category_id: &str,
) -> CoreResult<usize> {
    // Only categorize real uncategorized spending — never transfers (invariant),
    // never income, never already-categorized rows.
    let ids: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT id FROM transactions \
             WHERE category_id IS NULL AND is_transfer = 0 AND amount_cents < 0 \
               AND lower(merchant_raw) LIKE lower(?1)",
        )?;
        let rows = stmt.query_map(params![pattern], |r| r.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if ids.is_empty() {
        return Ok(0);
    }
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    {
        let mut set_cat = tx.prepare_cached(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
        )?;
        let mut record = tx.prepare_cached(
            "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) \
             VALUES(?1, ?2, ?3, 'rule', 1.0, NULL, ?4)",
        )?;
        for id in &ids {
            set_cat.execute(params![category_id, id])?;
            record.execute(params![Uuid::new_v4().to_string(), id, category_id, now])?;
        }
    }
    tx.commit()?;
    Ok(ids.len())
}

pub fn set_enabled(conn: &mut Connection, id: &str, enabled: bool) -> CoreResult<()> {
    conn.execute(
        "UPDATE rules SET enabled = ?1 WHERE id = ?2",
        params![enabled as i64, id],
    )?;
    Ok(())
}

/// Apply every active `transfer`/`settle_up`-treatment rule to still-undecided
/// transactions, so a counterparty verdict ruled once (see
/// `repos::transactions::apply_verdict_to_matching`) persists automatically to
/// every future import of that person — the transfer-review card never has to
/// re-ask. "Undecided" mirrors [`apply_verdict_to_matching`]'s own sibling
/// filter: `transfer_override IS NULL` (an explicit per-row verdict — transfer,
/// settle-up, or real — always sets it non-NULL, so this alone guarantees a
/// treatment rule never overturns a user's direct call, and makes re-running
/// this on every import a no-op for rows it already treated); PLUS
/// `transfer_peer_id IS NULL`, because `set_counterparty_verdict` unlinks only
/// the ONE row it's called on — applying a rule to just one leg of a pair
/// `pair_transfers` already linked would leave the peer dangling with a
/// half-severed link (asymmetric corruption); PLUS `category_id IS NULL`,
/// because a manually categorized row is already decided and a `transfer` verdict
/// would silently wipe that category. Both signs are matched (a settle-up/
/// transfer counterparty has inflows AND outflows); unlike
/// [`apply_to_uncategorized`] this does NOT filter by `amount_cents < 0`.
/// Applies each match via `transactions::set_counterparty_verdict` so the full
/// verdict semantics apply uniformly. Returns the number of rows treated.
pub fn apply_treatment_rules(conn: &mut Connection) -> CoreResult<u32> {
    let treatment_rules: Vec<(String, String)> = {
        let mut stmt = conn.prepare(
            "SELECT pattern, treatment FROM rules \
             WHERE enabled = 1 AND treatment IN ('transfer', 'settle_up')",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };

    let mut count = 0u32;
    for (pattern, treatment) in treatment_rules {
        let ids: Vec<String> = {
            // Scoped to the transfer-review vocabulary (same predicate the
            // review card itself uses) so a persisted rule never sweeps rows
            // that never appeared on the card, e.g. "%joe%" catching Trader
            // Joe's groceries alongside a Joe e-transfer.
            let sql = format!(
                "SELECT id FROM transactions t \
                 WHERE lower(t.merchant_raw) LIKE lower(?1) AND {}",
                crate::categorize::transfer_review_predicate("t")
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![pattern], |r| r.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let verdict = if treatment == "transfer" {
            crate::repos::transactions::Verdict::Transfer
        } else {
            crate::repos::transactions::Verdict::SettleUp
        };
        for id in ids {
            crate::repos::transactions::set_counterparty_verdict(conn, &id, verdict)?;
            count += 1;
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("r.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn apply_to_uncategorized_backfills_history_but_not_transfers_or_income() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)", []).unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('housing','g1','Housing','#f00',0)", []).unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES('chk','You','B','Checking','C','CAD','#111','manual',datetime('now'))",
            [],
        ).unwrap();
        // Two rent e-transfers (uncategorized expense), a same-recipient transfer
        // leg, and an income row — only the two expenses should be categorized.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) VALUES\
             ('r1','chk','2026-05-02T12:00:00Z',-120000,'INTERAC e-Transfer To: LANDLORD PROPERTIES',0,'cleared',datetime('now')),\
             ('r2','chk','2026-06-02T12:00:00Z',-120000,'INTERAC e-Transfer To: LANDLORD PROPERTIES',0,'cleared',datetime('now')),\
             ('tf','chk','2026-06-02T12:00:00Z', 120000,'INTERAC e-Transfer From LANDLORD refund',1,'cleared',datetime('now')),\
             ('in','chk','2026-06-03T12:00:00Z', 500000,'LANDLORD PROPERTIES DEPOSIT',0,'cleared',datetime('now'))",
            [],
        ).unwrap();

        let n = apply_to_uncategorized(&mut conn, "%landlord properties%", "housing").unwrap();
        assert_eq!(n, 2, "both rent expenses categorized");
        let housing: i64 = conn.query_row(
            "SELECT COUNT(*) FROM transactions WHERE category_id='housing'", [], |r| r.get(0)).unwrap();
        assert_eq!(housing, 2);
        // Transfer leg and income row untouched.
        let tf_cat: Option<String> = conn.query_row("SELECT category_id FROM transactions WHERE id='tf'", [], |r| r.get(0)).unwrap();
        let in_cat: Option<String> = conn.query_row("SELECT category_id FROM transactions WHERE id='in'", [], |r| r.get(0)).unwrap();
        assert_eq!(tf_cat, None, "transfer leg never categorized");
        assert_eq!(in_cat, None, "income row (positive amount) not categorized");
    }

    #[test]
    fn insert_and_list_active_rules() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();

        let rule = NewRule {
            pattern: "%amazon%".to_string(),
            category_id: "cat1".to_string(),
            source: "user".to_string(),
            treatment: "categorize".to_string(),
        };
        let r = insert(&mut conn, rule).unwrap();
        assert_eq!(r.pattern, "%amazon%");

        let active = list_active(&mut conn).unwrap();
        assert_eq!(active.len(), 1);

        set_enabled(&mut conn, &r.id, false).unwrap();
        let active2 = list_active(&mut conn).unwrap();
        assert_eq!(active2.len(), 0);
    }

    #[test]
    fn apply_treatment_rules_settles_matching_future_rows() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','B','Checking','C','CAD','#111','manual',datetime('now'))",
            [],
        )
        .unwrap();
        // A settle_up rule for "joe", persisted from an earlier bulk verdict.
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at,treatment) \
             VALUES('r1','%joe%','',1,'user','2026-01-01T00:00:00Z','settle_up')",
            [],
        )
        .unwrap();
        // A fresh import brings in both an inflow and an outflow leg for joe,
        // neither decided yet.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,transfer_override) VALUES\
             ('in1','chk','2026-07-01T12:00:00Z', 30000,'e-transfer joe','cleared','2026-07-01T12:00:00Z',NULL),\
             ('out1','chk','2026-07-02T12:00:00Z',-50000,'e-transfer joe','cleared','2026-07-02T12:00:00Z',NULL)",
            [],
        )
        .unwrap();

        let n = apply_treatment_rules(&mut conn).unwrap();
        assert_eq!(n, 2, "both the inflow and outflow leg are treated");

        let (in_settled, out_settled): (i64, i64) = conn
            .query_row(
                "SELECT (SELECT settle_up FROM transactions WHERE id='in1'), \
                        (SELECT settle_up FROM transactions WHERE id='out1')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!((in_settled, out_settled), (1, 1), "both signs settled — not filtered by amount sign");
    }

    #[test]
    fn apply_treatment_rules_respects_explicit_override() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','B','Checking','C','CAD','#111','manual',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at,treatment) \
             VALUES('r1','%joe%','',1,'user','2026-01-01T00:00:00Z','transfer')",
            [],
        )
        .unwrap();
        // The user already ruled this exact row before the treatment rule ran.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,transfer_override,is_transfer) VALUES\
             ('t1','chk','2026-07-01T12:00:00Z',-50000,'e-transfer joe','cleared','2026-07-01T12:00:00Z',1,1)",
            [],
        )
        .unwrap();

        let n = apply_treatment_rules(&mut conn).unwrap();
        assert_eq!(n, 0, "an explicit per-row verdict always wins over a treatment rule");
    }

    #[test]
    fn apply_treatment_rules_never_touches_an_already_paired_leg() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','B','Checking','C','CAD','#111','manual',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at,treatment) \
             VALUES('r1','%joe%','',1,'user','2026-01-01T00:00:00Z','settle_up')",
            [],
        )
        .unwrap();
        // pair_transfers already linked these two legs (transfer_peer_id set,
        // is_transfer=1) before the treatment rule ran; transfer_override is
        // still NULL on both — the pairing pass never sets it.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,transfer_override,is_transfer,transfer_peer_id) VALUES\
             ('p1','chk','2026-07-01T12:00:00Z',-50000,'e-transfer joe','cleared','2026-07-01T12:00:00Z',NULL,1,'p2'),\
             ('p2','chk','2026-07-01T12:00:00Z', 50000,'e-transfer joe','cleared','2026-07-01T12:00:00Z',NULL,1,'p1')",
            [],
        )
        .unwrap();

        let n = apply_treatment_rules(&mut conn).unwrap();
        assert_eq!(
            n, 0,
            "an already-paired leg is left alone — settling only one side would dangle the peer's link"
        );
        let (p1_peer, p2_peer): (Option<String>, Option<String>) = conn
            .query_row(
                "SELECT (SELECT transfer_peer_id FROM transactions WHERE id='p1'), \
                        (SELECT transfer_peer_id FROM transactions WHERE id='p2')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(p1_peer.as_deref(), Some("p2"), "pair link intact on both sides");
        assert_eq!(p2_peer.as_deref(), Some("p1"), "pair link intact on both sides");
    }

    #[test]
    fn apply_treatment_rules_does_not_sweep_non_transfer_lookalikes() {
        // A persisted "%joe%" settle_up rule must only auto-treat rows that
        // actually look like a transfer (the transfer-review vocabulary) —
        // not every future uncategorized row whose merchant contains "joe",
        // e.g. Trader Joe's groceries. Otherwise every import silently
        // mis-nets Trader Joe's purchases as settled-up.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','B','Checking','C','CAD','#111','manual',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rules(id,pattern,category_id,enabled,source,created_at,treatment) \
             VALUES('r1','%joe%','',1,'user','2026-01-01T00:00:00Z','settle_up')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('e1','chk','2026-07-01T12:00:00Z',-50000,'Internet Banking E-TRANSFER 111 Joe','cleared','2026-07-01T12:00:00Z'),\
             ('g1','chk','2026-07-02T12:00:00Z',-8000,'TRADER JOE''S #123','cleared','2026-07-02T12:00:00Z')",
            [],
        )
        .unwrap();

        let n = apply_treatment_rules(&mut conn).unwrap();
        assert_eq!(n, 1, "only the e-transfer-vocab row is treated");

        let e1_settled: i64 = conn
            .query_row("SELECT settle_up FROM transactions WHERE id = 'e1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(e1_settled, 1, "the e-transfer is settled up");

        let (g1_settled, g1_cat): (i64, Option<String>) = conn
            .query_row(
                "SELECT settle_up, category_id FROM transactions WHERE id = 'g1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(g1_settled, 0, "Trader Joe's groceries are left alone");
        assert!(g1_cat.is_none(), "Trader Joe's groceries stay uncategorized, not settled");
    }

    #[test]
    fn insert_and_list_active_rule_treatment() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('cat1','g1','Food','#f00',0)", []).unwrap();

        let rule = NewRule {
            pattern: "%joe%".to_string(),
            category_id: "cat1".to_string(),
            source: "user".to_string(),
            treatment: "settle_up".to_string(),
        };
        let r = insert(&mut conn, rule).unwrap();
        assert_eq!(r.treatment, "settle_up");

        let active = list_active(&mut conn).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].treatment, "settle_up");
    }
}
