//! Restoration envelopes — "what does it take to put that money back?"
//!
//! A notional running tab against an intention, not a claim about where
//! physical dollars are sitting. Once withdrawn money lands in checking it
//! mixes with everything already there, so "which dollars left next" is not a
//! fact in the data; any answer would be an artefact of a convention we
//! invented. This models only the legs that are genuinely knowable.
//!
//! The whole thing is designed backward from one sentence:
//!
//! > "To restore your $10,000: move the $9,500 you still have, collect $500,
//! > replace $0 you spent."
//!
//! Of those, three numbers are reliable and are computed here: what is left to
//! restore, what is collectable from a person, and what the user must therefore
//! fund themselves. "What you still have" is deliberately NOT asserted — see
//! [`RestorationStatus::still_held_ceiling_cents`].

use crate::error::CoreResult;
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;

/// An envelope stops being "in flight" and starts being clutter somewhere
/// around here. The issue this implements is explicit that features needing
/// ongoing manual attribution rarely survive past the first month, so the
/// design nags toward reconcile-and-close rather than letting them pile up.
pub const STALE_AFTER_DAYS: i64 = 45;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestorationEnvelope {
    pub id: String,
    pub label: String,
    pub source_account_id: Option<String>,
    pub destination_account_id: Option<String>,
    pub original_cents: i64,
    pub opened_on: String,
    pub counterparty_pattern: Option<String>,
    pub closed_at: Option<String>,
    pub note: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestorationLeg {
    pub id: String,
    pub envelope_id: String,
    pub transaction_id: Option<String>,
    pub amount_cents: i64,
    pub noted_on: String,
}

/// Everything needed to say the sentence, and nothing that would need to be
/// asserted without evidence.
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestorationStatus {
    pub envelope: RestorationEnvelope,
    pub legs: Vec<RestorationLeg>,
    /// Sum of everything attributed as already gone back.
    pub restored_cents: i64,
    /// Original minus restored. The headline number.
    pub left_to_restore_cents: i64,
    /// What a person still owes, from the counterparty tab. Zero when the
    /// envelope names nobody — an unlinked envelope has nothing collectable,
    /// and guessing across every counterparty would produce a confident wrong
    /// figure.
    pub collectable_cents: i64,
    /// What the user has to find themselves: left-to-restore minus collectable.
    pub fund_yourself_cents: i64,
    /// An UPPER BOUND on what could still be sitting in the destination
    /// account, never a claim that it is.
    ///
    /// The naive "still remaining" figure degrades silently: withdraw in
    /// January, flag a couple of legs, and it will still confidently report the
    /// same number in July after six months of unrelated churn. At that point
    /// it is not measuring anything.
    ///
    /// The account's lowest end-of-day balance since the withdrawal is a real
    /// bound instead — if checking dipped to $2,000 in March, the user
    /// demonstrably is not still holding $9,500. `None` when the destination is
    /// unknown or its balance cannot be reconstructed, which is the honest
    /// answer rather than a guess.
    pub still_held_ceiling_cents: Option<i64>,
    /// Why the ceiling is what it is, or why there isn't one.
    pub still_held_basis: String,
    pub days_open: i64,
    /// Open long enough that it should be reconciled and closed.
    pub stale: bool,
}

fn row_to_envelope(r: &rusqlite::Row<'_>) -> rusqlite::Result<RestorationEnvelope> {
    Ok(RestorationEnvelope {
        id: r.get(0)?,
        label: r.get(1)?,
        source_account_id: r.get(2)?,
        destination_account_id: r.get(3)?,
        original_cents: r.get(4)?,
        opened_on: r.get(5)?,
        counterparty_pattern: r.get(6)?,
        closed_at: r.get(7)?,
        note: r.get(8)?,
        created_at: r.get(9)?,
    })
}

const SELECT_COLS: &str = "id, label, source_account_id, destination_account_id, \
     original_cents, opened_on, counterparty_pattern, closed_at, note, created_at";

pub struct NewRestorationEnvelope {
    pub label: String,
    pub source_account_id: Option<String>,
    pub destination_account_id: Option<String>,
    pub original_cents: i64,
    pub opened_on: String,
    pub counterparty_pattern: Option<String>,
    pub note: Option<String>,
}

pub fn create(conn: &Connection, input: NewRestorationEnvelope) -> CoreResult<RestorationEnvelope> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO restoration_envelopes(id,label,source_account_id,destination_account_id,\
          original_cents,opened_on,counterparty_pattern,closed_at,note,created_at) \
         VALUES(?1,?2,?3,?4,?5,?6,?7,NULL,?8,?9)",
        params![
            id,
            input.label,
            input.source_account_id,
            input.destination_account_id,
            // A negative original would invert every downstream number.
            input.original_cents.max(0),
            input.opened_on,
            input.counterparty_pattern,
            input.note,
            now
        ],
    )?;
    get(conn, &id)?.ok_or_else(|| {
        crate::error::CoreError::InvalidState("envelope vanished after insert".into())
    })
}

pub fn get(conn: &Connection, id: &str) -> CoreResult<Option<RestorationEnvelope>> {
    let mut stmt =
        conn.prepare(&format!("SELECT {SELECT_COLS} FROM restoration_envelopes WHERE id = ?1"))?;
    let mut rows = stmt.query_map(params![id], |r| row_to_envelope(r))?;
    Ok(rows.next().transpose()?)
}

/// Open envelopes, oldest first — the one most in need of reconciling leads.
pub fn list_open(conn: &Connection) -> CoreResult<Vec<RestorationEnvelope>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} FROM restoration_envelopes \
         WHERE closed_at IS NULL ORDER BY opened_on ASC, label ASC"
    ))?;
    let rows = stmt.query_map([], |r| row_to_envelope(r))?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Mark an envelope reconciled. Idempotent: closing a closed envelope keeps the
/// original timestamp rather than moving the goalposts.
pub fn close(conn: &Connection, id: &str) -> CoreResult<()> {
    conn.execute(
        "UPDATE restoration_envelopes SET closed_at = ?1 \
         WHERE id = ?2 AND closed_at IS NULL",
        params![Utc::now().to_rfc3339(), id],
    )?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM restoration_envelopes WHERE id = ?1", params![id])?;
    Ok(())
}

/// Record money that has gone back. `amount_cents` is normalised to positive:
/// a restoration is an inflow to the pot however the caller phrased it.
pub fn add_leg(
    conn: &Connection,
    envelope_id: &str,
    amount_cents: i64,
    noted_on: &str,
    transaction_id: Option<&str>,
) -> CoreResult<RestorationLeg> {
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO restoration_legs(id,envelope_id,transaction_id,amount_cents,noted_on,created_at) \
         VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            id,
            envelope_id,
            transaction_id,
            amount_cents.abs(),
            noted_on,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(RestorationLeg {
        id,
        envelope_id: envelope_id.to_string(),
        transaction_id: transaction_id.map(str::to_string),
        amount_cents: amount_cents.abs(),
        noted_on: noted_on.to_string(),
    })
}

pub fn remove_leg(conn: &Connection, leg_id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM restoration_legs WHERE id = ?1", params![leg_id])?;
    Ok(())
}

pub fn list_legs(conn: &Connection, envelope_id: &str) -> CoreResult<Vec<RestorationLeg>> {
    let mut stmt = conn.prepare(
        "SELECT id, envelope_id, transaction_id, amount_cents, noted_on \
         FROM restoration_legs WHERE envelope_id = ?1 ORDER BY noted_on ASC",
    )?;
    let rows = stmt.query_map(params![envelope_id], |r| {
        Ok(RestorationLeg {
            id: r.get(0)?,
            envelope_id: r.get(1)?,
            transaction_id: r.get(2)?,
            amount_cents: r.get(3)?,
            noted_on: r.get(4)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value.get(..10).unwrap_or(value), "%Y-%m-%d").ok()
}

/// The three reliable numbers, plus an honest bound on the fourth.
pub fn status(conn: &mut Connection, envelope_id: &str) -> CoreResult<Option<RestorationStatus>> {
    let Some(envelope) = get(conn, envelope_id)? else {
        return Ok(None);
    };
    let legs = list_legs(conn, envelope_id)?;
    let restored: i64 = legs.iter().map(|l| l.amount_cents).sum();
    let left = (envelope.original_cents - restored).max(0);

    // Collectable comes straight from the counterparty tab, which is derived
    // fresh from real transactions — so it cannot drift the way a stored figure
    // would.
    let collectable = match envelope.counterparty_pattern.as_deref() {
        Some(pattern) => crate::repos::transactions::counterparty_position(conn, pattern)?
            .map(|p| p.owed_to_user_cents())
            .unwrap_or(0),
        None => 0,
    };
    // Never claim more is collectable than is actually outstanding.
    let collectable = collectable.min(left);

    let (ceiling, basis) = still_held_ceiling(conn, &envelope);

    let days_open = parse_date(&envelope.opened_on)
        .map(|d| (Utc::now().date_naive() - d).num_days().max(0))
        .unwrap_or(0);

    Ok(Some(RestorationStatus {
        restored_cents: restored,
        left_to_restore_cents: left,
        collectable_cents: collectable,
        fund_yourself_cents: (left - collectable).max(0),
        still_held_ceiling_cents: ceiling,
        still_held_basis: basis,
        days_open,
        stale: envelope.closed_at.is_none() && days_open >= STALE_AFTER_DAYS,
        envelope,
        legs,
    }))
}

/// The destination account's lowest end-of-day balance since the withdrawal.
///
/// This is the only defensible statement about "what you still have": money
/// cannot still be sitting there if the balance went below it at any point.
fn still_held_ceiling(
    conn: &mut Connection,
    envelope: &RestorationEnvelope,
) -> (Option<i64>, String) {
    let Some(account_id) = envelope.destination_account_id.as_deref() else {
        return (
            None,
            "No destination account recorded, so there is no balance history to bound this against."
                .to_string(),
        );
    };
    let opened = envelope.opened_on.get(..10).unwrap_or(&envelope.opened_on);
    let Ok(timeline) = crate::repos::accounts::balance_timeline(conn, account_id, Some(opened))
    else {
        return (
            None,
            "That account's balance history could not be read.".to_string(),
        );
    };
    match timeline.trough {
        Some(trough) => {
            let bound = trough.balance_cents.max(0);
            (
                Some(bound),
                format!(
                    "{} dipped to its lowest point on {}, so no more than that can still be sitting there.",
                    timeline.account_name, trough.date
                ),
            )
        }
        None => (
            None,
            format!(
                "{}'s balance cannot be reconstructed, so there is no verifiable ceiling.",
                timeline.account_name
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("restore.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_checking(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) \
             VALUES(?1,'Me','Bank','Checking','Everyday','USD','#fff','2026-01-01T00:00:00Z')",
            params![id],
        )
        .unwrap();
    }

    fn open_envelope(conn: &Connection, original: i64, counterparty: Option<&str>) -> RestorationEnvelope {
        create(
            conn,
            NewRestorationEnvelope {
                label: "Car fund".into(),
                source_account_id: None,
                destination_account_id: None,
                original_cents: original,
                opened_on: "2026-01-10".into(),
                counterparty_pattern: counterparty.map(str::to_string),
                note: None,
            },
        )
        .unwrap()
    }

    /// The issue's worked example, end to end.
    #[test]
    fn it_answers_what_it_takes_to_put_the_money_back() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_checking(&conn, "chk");
        // $10,000 came out of the car fund; $3,000 went to a friend, $2,500 came back.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('l1','chk','2026-01-15T12:00:00Z',-300000,'E-TRANSFER 1 Joe','cleared','2026-01-15T12:00:00Z'),\
             ('r1','chk','2026-02-20T12:00:00Z', 250000,'E-TRANSFER 2 Joe','cleared','2026-02-20T12:00:00Z')",
            [],
        )
        .unwrap();

        let env = open_envelope(&conn, 1_000_000, Some("joe"));
        let st = status(&mut conn, &env.id).unwrap().unwrap();

        assert_eq!(st.left_to_restore_cents, 1_000_000, "nothing has gone back yet");
        assert_eq!(st.collectable_cents, 50_000, "$500 outstanding with Joe");
        assert_eq!(
            st.fund_yourself_cents, 950_000,
            "the rest is the user's own to move or replace"
        );
    }

    #[test]
    fn restoring_money_reduces_what_is_left_and_the_arithmetic_stays_consistent() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let env = open_envelope(&conn, 1_000_000, None);

        add_leg(&conn, &env.id, 400_000, "2026-02-01", None).unwrap();
        add_leg(&conn, &env.id, 100_000, "2026-03-01", None).unwrap();

        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.restored_cents, 500_000);
        assert_eq!(st.left_to_restore_cents, 500_000);
        // The invariant the issue states: original = restored + still owing.
        assert_eq!(
            st.envelope.original_cents,
            st.restored_cents + st.left_to_restore_cents
        );
        assert_eq!(st.fund_yourself_cents, st.left_to_restore_cents - st.collectable_cents);
    }

    #[test]
    fn over_restoring_does_not_produce_a_negative_remainder() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let env = open_envelope(&conn, 100_000, None);
        add_leg(&conn, &env.id, 250_000, "2026-02-01", None).unwrap();

        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.left_to_restore_cents, 0);
        assert_eq!(st.fund_yourself_cents, 0);
    }

    #[test]
    fn a_restoration_leg_counts_however_the_caller_signs_it() {
        // "Money that went back" is an inflow to the pot whether the caller
        // passes it as positive or as the negative side of a transfer.
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let env = open_envelope(&conn, 100_000, None);
        add_leg(&conn, &env.id, -30_000, "2026-02-01", None).unwrap();

        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.restored_cents, 30_000);
    }

    #[test]
    fn collectable_never_exceeds_what_is_actually_outstanding() {
        // A friend owing more than remains on the envelope must not make the
        // envelope claim a negative "fund yourself".
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_checking(&conn, "chk");
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('l1','chk','2026-01-15T12:00:00Z',-900000,'E-TRANSFER 1 Joe','cleared','2026-01-15T12:00:00Z')",
            [],
        )
        .unwrap();

        let env = open_envelope(&conn, 100_000, Some("joe"));
        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.collectable_cents, 100_000, "capped at what is left");
        assert_eq!(st.fund_yourself_cents, 0);
    }

    #[test]
    fn an_envelope_naming_nobody_collects_nothing_rather_than_guessing() {
        // Summing every counterparty would produce a confident wrong figure
        // about money that has nothing to do with this envelope.
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_checking(&conn, "chk");
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('l1','chk','2026-01-15T12:00:00Z',-300000,'E-TRANSFER 1 Joe','cleared','2026-01-15T12:00:00Z')",
            [],
        )
        .unwrap();

        let env = open_envelope(&conn, 1_000_000, None);
        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.collectable_cents, 0);
        assert_eq!(st.fund_yourself_cents, 1_000_000);
    }

    #[test]
    fn what_you_still_have_is_bounded_by_the_account_low_point_not_asserted() {
        // The naive "still remaining" figure would keep reporting the original
        // amount forever. The account's trough since the withdrawal is a real
        // bound: if it dipped to $2,000, the user is demonstrably not holding
        // $9,500.
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        seed_checking(&conn, "chk");
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('d1','chk','2026-01-10T12:00:00Z', 1000000,'Withdrawal landing','cleared','2026-01-10T12:00:00Z'),\
             ('s1','chk','2026-02-01T12:00:00Z', -800000,'Big spend','cleared','2026-02-01T12:00:00Z'),\
             ('i1','chk','2026-03-01T12:00:00Z',  600000,'Payroll','cleared','2026-03-01T12:00:00Z')",
            [],
        )
        .unwrap();

        let env = create(
            &conn,
            NewRestorationEnvelope {
                label: "Car fund".into(),
                source_account_id: None,
                destination_account_id: Some("chk".into()),
                original_cents: 1_000_000,
                opened_on: "2026-01-10".into(),
                counterparty_pattern: None,
                note: None,
            },
        )
        .unwrap();

        let st = status(&mut conn, &env.id).unwrap().unwrap();
        let ceiling = st.still_held_ceiling_cents.expect("a reconstructable account has a trough");
        assert!(
            ceiling < 1_000_000,
            "the balance dipped, so the ceiling must be below the original: {ceiling}"
        );
        assert!(st.still_held_basis.contains("lowest point"));
    }

    #[test]
    fn no_destination_account_means_no_ceiling_rather_than_a_guess() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let env = open_envelope(&conn, 1_000_000, None);
        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.still_held_ceiling_cents, None);
        assert!(st.still_held_basis.contains("No destination account"));
    }

    #[test]
    fn an_envelope_left_open_too_long_is_flagged_for_reconciling() {
        // Features needing ongoing manual attribution rot. This one nags.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let old = (Utc::now().date_naive() - chrono::Duration::days(STALE_AFTER_DAYS + 5))
            .format("%Y-%m-%d")
            .to_string();
        let env = create(
            &conn,
            NewRestorationEnvelope {
                label: "Old one".into(),
                source_account_id: None,
                destination_account_id: None,
                original_cents: 100_000,
                opened_on: old,
                counterparty_pattern: None,
                note: None,
            },
        )
        .unwrap();
        drop(conn);
        let mut conn = db.get().unwrap();

        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert!(st.stale);
        assert!(st.days_open >= STALE_AFTER_DAYS);
    }

    #[test]
    fn a_closed_envelope_stops_being_nagged_about_and_leaves_the_open_list() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let old = (Utc::now().date_naive() - chrono::Duration::days(STALE_AFTER_DAYS + 5))
            .format("%Y-%m-%d")
            .to_string();
        let env = create(
            &conn,
            NewRestorationEnvelope {
                label: "Done".into(),
                source_account_id: None,
                destination_account_id: None,
                original_cents: 100_000,
                opened_on: old,
                counterparty_pattern: None,
                note: None,
            },
        )
        .unwrap();
        assert_eq!(list_open(&conn).unwrap().len(), 1);

        close(&conn, &env.id).unwrap();
        assert!(list_open(&conn).unwrap().is_empty());

        drop(conn);
        let mut conn = db.get().unwrap();
        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert!(!st.stale, "a reconciled envelope is not clutter");
        assert!(st.envelope.closed_at.is_some());
    }

    #[test]
    fn closing_twice_keeps_the_first_timestamp() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let env = open_envelope(&conn, 100_000, None);
        close(&conn, &env.id).unwrap();
        let first = get(&conn, &env.id).unwrap().unwrap().closed_at;
        close(&conn, &env.id).unwrap();
        assert_eq!(get(&conn, &env.id).unwrap().unwrap().closed_at, first);
    }

    #[test]
    fn deleting_an_envelope_takes_its_legs_with_it() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        let env = open_envelope(&conn, 100_000, None);
        add_leg(&conn, &env.id, 10_000, "2026-02-01", None).unwrap();
        assert_eq!(list_legs(&conn, &env.id).unwrap().len(), 1);

        delete(&conn, &env.id).unwrap();
        assert!(get(&conn, &env.id).unwrap().is_none());
        assert!(list_legs(&conn, &env.id).unwrap().is_empty());
    }

    #[test]
    fn an_unknown_envelope_has_no_status_rather_than_a_zeroed_one() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        assert!(status(&mut conn, "nope").unwrap().is_none());
    }

    #[test]
    fn a_negative_original_is_clamped_rather_than_inverting_everything() {
        let (_d, db) = fresh();
        let mut conn = db.get().unwrap();
        let env = open_envelope(&conn, -500_000, None);
        let st = status(&mut conn, &env.id).unwrap().unwrap();
        assert_eq!(st.envelope.original_cents, 0);
        assert_eq!(st.left_to_restore_cents, 0);
    }
}
