//! Household members and account ownership.
//!
//! An account with 2+ owners is a JOINT account — jointness is derived from
//! `account_owners`, never stored as a flag. `accounts.owner` (legacy TEXT)
//! is kept in sync as a display string ("Koushik & Swathi", "Household" when
//! unassigned) so older read paths and AI context stay meaningful.

use crate::error::{CoreError, CoreResult};
use crate::models::{AccountOwner, HouseholdMember, OwnerShare};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list_members(conn: &mut Connection) -> CoreResult<Vec<HouseholdMember>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at, is_self FROM household_members ORDER BY created_at, name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(HouseholdMember {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            created_at: r.get(3)?,
            is_self: r.get::<_, i64>(4)? != 0,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Mark exactly one member as the "self" (the person operating this install),
/// clearing the flag on every other member. Idempotent. Passing a member id
/// that doesn't exist clears self entirely (no-op set). At most one member is
/// ever self — enforced here rather than by a DB constraint.
pub fn set_self_member(conn: &mut Connection, member_id: &str) -> CoreResult<()> {
    let tx = conn.transaction()?;
    tx.execute("UPDATE household_members SET is_self = 0", [])?;
    tx.execute(
        "UPDATE household_members SET is_self = 1 WHERE id = ?1",
        params![member_id],
    )?;
    tx.commit()?;
    Ok(())
}

/// The current operator ("self") member, if one is set.
pub fn self_member(conn: &mut Connection) -> CoreResult<Option<HouseholdMember>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at, is_self FROM household_members WHERE is_self = 1 LIMIT 1",
    )?;
    let mut rows = stmt.query_map([], |r| {
        Ok(HouseholdMember {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            created_at: r.get(3)?,
            is_self: r.get::<_, i64>(4)? != 0,
        })
    })?;
    match rows.next() {
        Some(m) => Ok(Some(m?)),
        None => Ok(None),
    }
}

pub fn create_member(
    conn: &mut Connection,
    name: &str,
    color: Option<&str>,
) -> CoreResult<HouseholdMember> {
    let name = name.trim();
    if name.is_empty() {
        return Err(CoreError::InvalidState("member name must not be empty".into()));
    }
    // Names identify people in every owner picker — a duplicate would be
    // ambiguous everywhere. Surface a friendly error instead of a constraint hit.
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM household_members WHERE lower(name) = lower(?1)",
        params![name],
        |r| r.get(0),
    )?;
    if exists > 0 {
        return Err(CoreError::InvalidState(format!(
            "a household member named \"{name}\" already exists"
        )));
    }
    let member = HouseholdMember {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        color: color.map(|c| c.to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
        is_self: false,
    };
    conn.execute(
        "INSERT INTO household_members(id, name, color, created_at) VALUES(?1, ?2, ?3, ?4)",
        params![member.id, member.name, member.color, member.created_at],
    )?;
    Ok(member)
}

/// Delete a member. Their ownership rows cascade away; affected accounts'
/// display strings are refreshed (an account can become sole or unassigned).
pub fn delete_member(conn: &mut Connection, member_id: &str) -> CoreResult<()> {
    let affected: Vec<String> = {
        let mut stmt =
            conn.prepare("SELECT account_id FROM account_owners WHERE member_id = ?1")?;
        let rows = stmt.query_map(params![member_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };
    conn.execute(
        "DELETE FROM household_members WHERE id = ?1",
        params![member_id],
    )?;
    for account_id in affected {
        sync_owner_display(conn, &account_id)?;
    }
    Ok(())
}

/// The full (account, member) pair list — one call for the whole UI to derive
/// badges and attribution.
pub fn list_account_owners(conn: &mut Connection) -> CoreResult<Vec<AccountOwner>> {
    let mut stmt = conn.prepare("SELECT account_id, member_id, share_bps FROM account_owners")?;
    let rows = stmt.query_map([], |r| {
        Ok(AccountOwner {
            account_id: r.get(0)?,
            member_id: r.get(1)?,
            share_bps: r.get(2)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Replace the owner set for an account with explicit shares. `share_bps` None on
/// an owner ⇒ that owner falls back to an equal split. Shares need not sum to
/// 10000 — a recorded total below 100% leaves the remainder in the household
/// residual (the cross-app share owned by another person's separate app).
pub fn set_account_owner_shares(
    conn: &mut Connection,
    account_id: &str,
    owners: &[OwnerShare],
) -> CoreResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM account_owners WHERE account_id = ?1",
        params![account_id],
    )?;
    for o in owners {
        tx.execute(
            "INSERT OR IGNORE INTO account_owners(account_id, member_id, share_bps) VALUES(?1, ?2, ?3)",
            params![account_id, o.member_id, o.share_bps],
        )?;
    }
    tx.commit()?;
    sync_owner_display(conn, account_id)
}

/// The full (asset, member) ownership pair list — the manual-asset analogue of
/// [`list_account_owners`].
pub fn list_asset_owners(conn: &mut Connection) -> CoreResult<Vec<crate::models::AssetOwner>> {
    let mut stmt = conn.prepare("SELECT asset_id, member_id, share_bps FROM asset_owners")?;
    let rows = stmt.query_map([], |r| {
        Ok(crate::models::AssetOwner {
            asset_id: r.get(0)?,
            member_id: r.get(1)?,
            share_bps: r.get(2)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// Replace the owner set for a manual asset with explicit shares (see
/// [`set_account_owner_shares`] for the share semantics).
pub fn set_asset_owners(
    conn: &mut Connection,
    asset_id: &str,
    owners: &[OwnerShare],
) -> CoreResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM asset_owners WHERE asset_id = ?1",
        params![asset_id],
    )?;
    for o in owners {
        tx.execute(
            "INSERT OR IGNORE INTO asset_owners(asset_id, member_id, share_bps) VALUES(?1, ?2, ?3)",
            params![asset_id, o.member_id, o.share_bps],
        )?;
    }
    tx.commit()
        .map_err(crate::error::CoreError::from)
}

/// Replace the owner set for an account (empty = household/unassigned).
pub fn set_account_owners(
    conn: &mut Connection,
    account_id: &str,
    member_ids: &[String],
) -> CoreResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM account_owners WHERE account_id = ?1",
        params![account_id],
    )?;
    for member_id in member_ids {
        tx.execute(
            "INSERT OR IGNORE INTO account_owners(account_id, member_id) VALUES(?1, ?2)",
            params![account_id, member_id],
        )?;
    }
    tx.commit()?;
    sync_owner_display(conn, account_id)
}

/// Keep the legacy `accounts.owner` display string aligned with the owner set.
fn sync_owner_display(conn: &Connection, account_id: &str) -> CoreResult<()> {
    let names: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT m.name FROM account_owners ao \
             JOIN household_members m ON m.id = ao.member_id \
             WHERE ao.account_id = ?1 ORDER BY m.created_at, m.name",
        )?;
        let rows = stmt.query_map(params![account_id], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };
    let display = if names.is_empty() {
        "Household".to_string()
    } else {
        names.join(" & ")
    };
    conn.execute(
        "UPDATE accounts SET owner = ?1 WHERE id = ?2",
        params![display, account_id],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("h.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn insert_account(conn: &Connection, id: &str, name: &str) {
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES(?1,'Household','Tangerine','Savings',?2,'CAD','#4ADE80','manual','2024-01-01T00:00:00Z')",
            params![id, name],
        )
        .unwrap();
    }

    fn owner_display(conn: &Connection, id: &str) -> String {
        conn.query_row("SELECT owner FROM accounts WHERE id = ?1", params![id], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn joint_account_lifecycle_two_owners_then_back_to_sole() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        insert_account(&conn, "sav", "Tangerine Savings");

        let koushik = create_member(&mut conn, "Koushik", Some("#38BDF8")).unwrap();
        let swathi = create_member(&mut conn, "Swathi", Some("#F472B6")).unwrap();

        // Joint: two owners.
        set_account_owners(&mut conn, "sav", &[koushik.id.clone(), swathi.id.clone()]).unwrap();
        let owners = list_account_owners(&mut conn).unwrap();
        assert_eq!(owners.len(), 2, "joint account has two ownership rows");
        assert_eq!(owner_display(&conn, "sav"), "Koushik & Swathi");

        // Back to sole ownership.
        set_account_owners(&mut conn, "sav", &[koushik.id.clone()]).unwrap();
        assert_eq!(list_account_owners(&mut conn).unwrap().len(), 1);
        assert_eq!(owner_display(&conn, "sav"), "Koushik");

        // Unassigned = household.
        set_account_owners(&mut conn, "sav", &[]).unwrap();
        assert_eq!(list_account_owners(&mut conn).unwrap().len(), 0);
        assert_eq!(owner_display(&conn, "sav"), "Household");
    }

    #[test]
    fn member_names_are_unique_case_insensitively() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        create_member(&mut conn, "Swathi", None).unwrap();
        let dup = create_member(&mut conn, "  swathi ", None);
        assert!(dup.is_err(), "duplicate member names must be rejected");
        let blank = create_member(&mut conn, "   ", None);
        assert!(blank.is_err(), "blank names must be rejected");
    }

    #[test]
    fn deleting_a_member_cascades_ownership_and_refreshes_display() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        insert_account(&conn, "sav", "Tangerine Savings");
        let a = create_member(&mut conn, "Koushik", None).unwrap();
        let b = create_member(&mut conn, "Swathi", None).unwrap();
        set_account_owners(&mut conn, "sav", &[a.id.clone(), b.id.clone()]).unwrap();

        delete_member(&mut conn, &b.id).unwrap();

        assert_eq!(list_members(&mut conn).unwrap().len(), 1);
        let owners = list_account_owners(&mut conn).unwrap();
        assert_eq!(owners.len(), 1, "the deleted member's ownership rows cascade away");
        assert_eq!(owner_display(&conn, "sav"), "Koushik", "display refreshes to the survivor");
    }

    #[test]
    fn deleting_an_account_cascades_its_ownership_rows() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        insert_account(&conn, "sav", "Tangerine Savings");
        let a = create_member(&mut conn, "Koushik", None).unwrap();
        set_account_owners(&mut conn, "sav", &[a.id.clone()]).unwrap();

        conn.execute("DELETE FROM accounts WHERE id = 'sav'", []).unwrap();

        assert_eq!(list_account_owners(&mut conn).unwrap().len(), 0);
        assert_eq!(list_members(&mut conn).unwrap().len(), 1, "the member survives");
    }

    #[test]
    fn self_member_is_unique_and_movable() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let a = create_member(&mut conn, "Koushik", None).unwrap();
        let b = create_member(&mut conn, "Swathi", None).unwrap();
        assert!(self_member(&mut conn).unwrap().is_none(), "no self by default");
        assert!(!a.is_self, "create_member never marks self");

        set_self_member(&mut conn, &a.id).unwrap();
        assert_eq!(self_member(&mut conn).unwrap().unwrap().id, a.id);
        assert_eq!(
            list_members(&mut conn).unwrap().iter().filter(|m| m.is_self).count(),
            1
        );

        // Re-pointing self moves it — never two selves at once.
        set_self_member(&mut conn, &b.id).unwrap();
        assert_eq!(self_member(&mut conn).unwrap().unwrap().id, b.id);
        assert_eq!(
            list_members(&mut conn).unwrap().iter().filter(|m| m.is_self).count(),
            1
        );
    }

    #[test]
    fn operator_identity_flags_own_e_transfers_but_not_a_friends() {
        // F0: once the operator's name is known (a household member), the builtin
        // pass recognizes their OWN e-transfers as internal moves — but a
        // same-shaped transfer naming a DIFFERENT person stays income/expense
        // (genuinely ambiguous: reimbursement vs. rent — left for user review, NOT
        // silently flagged). This is the whole reason a real user's savings rate
        // is wrong until they enter their name.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        insert_account(&conn, "chq", "CIBC Chequing");
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
             VALUES('t_self','chq','2026-06-21T00:00:00Z',-300000,'INTERAC e-Transfer To: Koushik','cleared','2026-06-21T00:00:00Z')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) \
             VALUES('t_friend','chq','2026-06-22T00:00:00Z',-50000,'Internet Banking E-TRANSFER 105950894357 Swathi','cleared','2026-06-22T00:00:00Z')",
            [],
        ).unwrap();

        // No operator yet: the own e-transfer is not recognized as internal.
        crate::categorize::apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(transfer_flag(&conn, "t_self"), 0, "no operator ⇒ own e-transfer not flagged");

        // Configure the operator; re-running recognizes their own e-transfer only.
        let me = create_member(&mut conn, "Koushik", None).unwrap();
        set_self_member(&mut conn, &me.id).unwrap();
        crate::categorize::apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(transfer_flag(&conn, "t_self"), 1, "own e-transfer is an internal move");
        assert_eq!(transfer_flag(&conn, "t_friend"), 0, "a friend's e-transfer stays for review");
    }

    fn transfer_flag(conn: &Connection, id: &str) -> i64 {
        conn.query_row(
            "SELECT is_transfer FROM transactions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn set_owner_shares_persists_explicit_shares_for_accounts_and_assets() {
        use crate::models::OwnerShare;
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        insert_account(&conn, "chq", "Joint Chequing");
        let a = create_member(&mut conn, "Koushik", None).unwrap();
        let b = create_member(&mut conn, "Swathi", None).unwrap();

        set_account_owner_shares(
            &mut conn,
            "chq",
            &[
                OwnerShare { member_id: a.id.clone(), share_bps: Some(7000) },
                OwnerShare { member_id: b.id.clone(), share_bps: Some(3000) },
            ],
        )
        .unwrap();
        let owners = list_account_owners(&mut conn).unwrap();
        assert_eq!(owners.len(), 2);
        assert_eq!(
            owners.iter().find(|o| o.member_id == a.id).unwrap().share_bps,
            Some(7000)
        );
        assert_eq!(owner_display(&conn, "chq"), "Koushik & Swathi", "display still syncs");

        conn.execute(
            "INSERT INTO manual_assets(id,name,asset_type,value_cents,currency,created_at,updated_at) \
             VALUES('house','House','Real Estate',50000000,'CAD','2024-01-01','2024-01-01')",
            [],
        )
        .unwrap();
        set_asset_owners(
            &mut conn,
            "house",
            &[
                OwnerShare { member_id: a.id.clone(), share_bps: Some(6000) },
                // None ⇒ equal-split fallback (stored NULL).
                OwnerShare { member_id: b.id.clone(), share_bps: None },
            ],
        )
        .unwrap();
        let ao = list_asset_owners(&mut conn).unwrap();
        assert_eq!(ao.len(), 2);
        assert_eq!(ao.iter().find(|o| o.member_id == a.id).unwrap().share_bps, Some(6000));
        assert_eq!(ao.iter().find(|o| o.member_id == b.id).unwrap().share_bps, None);
    }
}
