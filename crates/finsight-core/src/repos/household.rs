//! Household members and account ownership.
//!
//! An account with 2+ owners is a JOINT account — jointness is derived from
//! `account_owners`, never stored as a flag. `accounts.owner` (legacy TEXT)
//! is kept in sync as a display string ("Koushik & Swathi", "Household" when
//! unassigned) so older read paths and AI context stay meaningful.

use crate::error::{CoreError, CoreResult};
use crate::models::{AccountOwner, HouseholdMember};
use rusqlite::{params, Connection};
use uuid::Uuid;

pub fn list_members(conn: &mut Connection) -> CoreResult<Vec<HouseholdMember>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, color, created_at FROM household_members ORDER BY created_at, name",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(HouseholdMember {
            id: r.get(0)?,
            name: r.get(1)?,
            color: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
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
    let mut stmt = conn.prepare("SELECT account_id, member_id FROM account_owners")?;
    let rows = stmt.query_map([], |r| {
        Ok(AccountOwner {
            account_id: r.get(0)?,
            member_id: r.get(1)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
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
}
