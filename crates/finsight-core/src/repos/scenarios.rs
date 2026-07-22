use crate::error::CoreResult;
use crate::forecast::{GoalInfo, Snapshot};
use crate::models::AccountType;
use crate::repos::{accounts, goals};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

/// The current financial baseline a what-if scenario projects against: spendable
/// balance (excluding credit and investments), 12-month average income/expense,
/// and goals. Single source of truth so the app's scenario command and the
/// Copilot's scenario tool compute — and compare against — the SAME baseline.
pub fn build_baseline(conn: &mut Connection) -> CoreResult<Snapshot> {
    let accts = accounts::list_summaries(conn)?;
    let balance: i64 = accts
        .iter()
        .filter(|a| !matches!(a.r#type, AccountType::Credit | AccountType::Investment))
        .map(|a| a.balance_cents)
        .sum();

    // Average over months actually elapsed since the first transaction in the
    // window (capped at 12), so a single lumpy import isn't the "typical" month.
    let (sum_income, sum_expense, span_months): (i64, i64, i64) = conn.query_row(
        "SELECT COALESCE(SUM(CASE WHEN amount_cents>0 AND settle_up=0 THEN amount_cents ELSE 0 END),0),\
                COALESCE(SUM(CASE WHEN settle_up=1 THEN -amount_cents \
                                  WHEN amount_cents<0 THEN -amount_cents \
                                  ELSE 0 END),0),\
                COALESCE(\
                  (CAST(strftime('%Y','now') AS INTEGER) - CAST(strftime('%Y', MIN(posted_at)) AS INTEGER)) * 12\
                  + (CAST(strftime('%m','now') AS INTEGER) - CAST(strftime('%m', MIN(posted_at)) AS INTEGER)) + 1,\
                  1)\
         FROM transactions\
         WHERE posted_at >= date('now','-12 months')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let am = span_months.clamp(1, 12);

    let goal_infos = goals::list(conn)?
        .into_iter()
        .map(|g| GoalInfo {
            name: g.name,
            remaining_cents: (g.target_cents - g.current_cents).max(0),
            monthly_cents: g.monthly_cents,
        })
        .collect();

    Ok(Snapshot {
        balance_cents: balance,
        avg_monthly_income_cents: sum_income / am,
        avg_monthly_expense_cents: sum_expense / am,
        goals: goal_infos,
    })
}

#[derive(Debug, Clone)]
pub struct ScenarioRow {
    pub id: String,
    pub description: String,
    pub result_json: String,
    pub created_at: String,
    /// The what-if params the scenario was run with. `None` for legacy rows
    /// saved before durable scenarios (V055) — those can be viewed but not
    /// recomputed or compared.
    pub params_json: Option<String>,
    /// The financial baseline (a serialized `forecast::Snapshot`) the scenario
    /// was computed against, for staleness detection.
    pub baseline_json: Option<String>,
    pub months: Option<i64>,
    /// Soft-archive marker; archived rows are hidden from the active list.
    pub archived_at: Option<String>,
    /// A REVISED set of what-if params (issue #73), stored alongside the
    /// immutable original so the scenario can be re-evaluated without rebuilding.
    /// `None` = no revision.
    pub revised_params_json: Option<String>,
}

const COLS: &str =
    "id, description, result_json, created_at, params_json, baseline_json, months, archived_at, revised_params_json";

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<ScenarioRow> {
    Ok(ScenarioRow {
        id: r.get(0)?,
        description: r.get(1)?,
        result_json: r.get(2)?,
        created_at: r.get(3)?,
        params_json: r.get(4)?,
        baseline_json: r.get(5)?,
        months: r.get(6)?,
        archived_at: r.get(7)?,
        revised_params_json: r.get(8)?,
    })
}

/// Insert a scenario. `params_json`/`baseline_json`/`months` may be `None` (a
/// bare result), but supplying them is what makes the scenario recomputable,
/// comparable, and staleness-checkable.
pub fn insert(
    conn: &mut Connection,
    description: &str,
    result_json: &str,
    params_json: Option<&str>,
    baseline_json: Option<&str>,
    months: Option<i64>,
) -> CoreResult<ScenarioRow> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO scenarios(id, description, result_json, created_at, params_json, baseline_json, months) \
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, description, result_json, now, params_json, baseline_json, months],
    )?;
    Ok(ScenarioRow {
        id,
        description: description.to_string(),
        result_json: result_json.to_string(),
        created_at: now,
        params_json: params_json.map(str::to_string),
        baseline_json: baseline_json.map(str::to_string),
        months,
        archived_at: None,
        revised_params_json: None,
    })
}

/// Set or clear a scenario's revised what-if params (issue #73). The original
/// params/result/baseline are never touched.
pub fn set_revised_params(
    conn: &mut Connection,
    id: &str,
    revised_params_json: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE scenarios SET revised_params_json = ?2 WHERE id = ?1",
        params![id, revised_params_json],
    )?;
    Ok(())
}

/// Active (non-archived) scenarios, newest first.
pub fn list(conn: &mut Connection) -> CoreResult<Vec<ScenarioRow>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLS} FROM scenarios WHERE archived_at IS NULL ORDER BY created_at DESC"
    ))?;
    let rows = stmt.query_map([], map_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// A single scenario by id (archived or not).
pub fn get(conn: &mut Connection, id: &str) -> CoreResult<Option<ScenarioRow>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLS} FROM scenarios WHERE id = ?1"))?;
    Ok(stmt.query_row(params![id], map_row).optional()?)
}

/// Duplicate a scenario into a fresh, independent row — its own id and
/// timestamp — so the copy can diverge without touching the original.
pub fn duplicate(conn: &mut Connection, id: &str) -> CoreResult<Option<ScenarioRow>> {
    let Some(src) = get(conn, id)? else {
        return Ok(None);
    };
    let copy = insert(
        conn,
        &format!("{} (copy)", src.description),
        &src.result_json,
        src.params_json.as_deref(),
        src.baseline_json.as_deref(),
        src.months,
    )?;
    Ok(Some(copy))
}

/// Archive or unarchive a scenario (soft delete — the row is preserved).
pub fn set_archived(conn: &mut Connection, id: &str, archived: bool) -> CoreResult<()> {
    let value: Option<String> = archived.then(|| Utc::now().to_rfc3339());
    conn.execute(
        "UPDATE scenarios SET archived_at = ?2 WHERE id = ?1",
        params![id, value],
    )?;
    Ok(())
}

pub fn delete(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM scenarios WHERE id = ?1", params![id])?;
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
        let db = Db::open(&dir.path().join("a.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn insert_list_delete_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let row = insert(&mut conn, "What if I buy a car?", r#"{"verdict":true}"#, Some(r#"{"p":1}"#), Some(r#"{"b":2}"#), Some(12)).unwrap();
        let listed = list(&mut conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].description, "What if I buy a car?");
        assert_eq!(listed[0].params_json.as_deref(), Some(r#"{"p":1}"#));
        assert_eq!(listed[0].months, Some(12));
        delete(&mut conn, &row.id).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 0);
    }

    #[test]
    fn revised_params_round_trip_leaves_original_untouched() {
        // Issue #73: a revision stores alongside the original — never over it.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let row = insert(&mut conn, "s", r#"{"verdict":true}"#, Some(r#"{"income":1}"#), Some(r#"{"b":2}"#), Some(24)).unwrap();
        assert!(row.revised_params_json.is_none());

        set_revised_params(&mut conn, &row.id, Some(r#"{"income":9}"#)).unwrap();
        let after = get(&mut conn, &row.id).unwrap().unwrap();
        assert_eq!(after.revised_params_json.as_deref(), Some(r#"{"income":9}"#));
        assert_eq!(after.params_json.as_deref(), Some(r#"{"income":1}"#), "original params untouched");
        assert_eq!(after.result_json, r#"{"verdict":true}"#, "original result untouched");

        set_revised_params(&mut conn, &row.id, None).unwrap();
        assert!(get(&mut conn, &row.id).unwrap().unwrap().revised_params_json.is_none(), "revision cleared");
    }

    #[test]
    fn legacy_result_only_rows_still_work() {
        // A pre-V055 row (no params/baseline) must round-trip, not crash.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        insert(&mut conn, "legacy", r#"{"verdict":true}"#, None, None, None).unwrap();
        let listed = list(&mut conn).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].params_json.is_none());
        assert!(listed[0].baseline_json.is_none());
    }

    #[test]
    fn duplicate_is_independent_and_archive_hides_from_active() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        let orig = insert(&mut conn, "Base", r#"{"verdict":true}"#, Some("{}"), Some("{}"), Some(6)).unwrap();

        let copy = duplicate(&mut conn, &orig.id).unwrap().unwrap();
        assert_ne!(copy.id, orig.id);
        assert_eq!(copy.description, "Base (copy)");
        assert_eq!(list(&mut conn).unwrap().len(), 2);

        // Archiving the copy hides it from the active list but preserves the row.
        set_archived(&mut conn, &copy.id, true).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 1);
        assert!(get(&mut conn, &copy.id).unwrap().unwrap().archived_at.is_some());

        // The original is untouched.
        set_archived(&mut conn, &copy.id, false).unwrap();
        assert_eq!(list(&mut conn).unwrap().len(), 2);
    }
}
