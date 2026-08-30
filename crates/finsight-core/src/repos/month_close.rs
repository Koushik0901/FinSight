use crate::error::CoreResult;
use rusqlite::{Connection, OptionalExtension};

/// Returns true when `month` ("YYYY-MM") has a completed close. Completed
/// months are soft-locked: edits that would drift the frozen snapshot require
/// an explicit Reopen before retrying.
pub fn is_locked(conn: &Connection, month: &str) -> CoreResult<bool> {
    let Some((y_str, m_str)) = month.split_once('-') else {
        return Ok(false);
    };
    let Ok(year) = y_str.parse::<i32>() else {
        return Ok(false);
    };
    let Ok(mon) = m_str.parse::<i32>() else {
        return Ok(false);
    };
    if !(1..=12).contains(&mon) {
        return Ok(false);
    }
    let exists = conn
        .query_row(
            "SELECT 1 FROM monthly_reviews WHERE year = ?1 AND month = ?2 AND status = 'completed'",
            rusqlite::params![year, mon],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    Ok(exists)
}

#[cfg(test)]
mod tests {
    use super::is_locked;
    use crate::testing::migrated_db;
    use crate::Db;

    #[test]
    fn is_locked_false_when_no_row() {
        let (_d, db) = migrated_db();
        let conn = db.get().unwrap();
        assert!(!is_locked(&conn, "2026-03").unwrap());
    }

    #[test]
    fn is_locked_true_when_completed() {
        let (_d, db) = migrated_db();
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO monthly_reviews(id, year, month, snapshot_json, created_at, status, completed_at) VALUES('r', 2026, 3, '{}', datetime('now'), 'completed', datetime('now'))",
            [],
        )
        .unwrap();
        assert!(is_locked(&conn, "2026-03").unwrap());
        assert!(!is_locked(&conn, "2026-04").unwrap());
    }

    #[test]
    fn is_locked_false_when_in_progress_or_skipped() {
        let (_d, db) = migrated_db();
        let conn = db.get().unwrap();
        for status in ["in_progress", "skipped"] {
            conn.execute("DELETE FROM monthly_reviews", []).unwrap();
            conn.execute(
                "INSERT INTO monthly_reviews(id, year, month, snapshot_json, created_at, status) VALUES(?1, 2026, 3, '{}', datetime('now'), ?2)",
                rusqlite::params![format!("r_{status}"), status],
            )
            .unwrap();
            assert!(
                !is_locked(&conn, "2026-03").unwrap(),
                "status={status} must not be locked"
            );
        }
    }

    #[test]
    fn is_locked_gracefully_handles_malformed_month() {
        let (_d, db) = migrated_db();
        let conn = db.get().unwrap();
        assert!(!is_locked(&conn, "").unwrap());
        assert!(!is_locked(&conn, "2026").unwrap());
        assert!(!is_locked(&conn, "2026-13").unwrap());
    }
}
