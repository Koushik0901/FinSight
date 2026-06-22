//! CRUD for the `imports` table — started at import begin, finished when complete.

use crate::error::CoreResult;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    Csv,
    Manual,
    Sample,
    SimpleFin,
}

impl ImportSource {
    fn as_db(&self) -> &'static str {
        match self {
            Self::Csv => "csv",
            Self::Manual => "manual",
            Self::Sample => "sample",
            Self::SimpleFin => "simplefin",
        }
    }

    pub fn from_db(s: &str) -> Self {
        match s {
            "manual" => Self::Manual,
            "sample" => Self::Sample,
            "simplefin" => Self::SimpleFin,
            _ => Self::Csv,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Import {
    pub id: String,
    pub source: ImportSource,
    pub filename: Option<String>,
    pub account_id: Option<String>,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub rows_imported: i64,
    pub rows_skipped_duplicates: i64,
    pub error: Option<String>,
}

/// Insert a new import row in started-but-not-finished state. Returns the id.
pub fn start(
    conn: &Connection,
    source: ImportSource,
    filename: Option<&str>,
    account_id: Option<&str>,
) -> CoreResult<String> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO imports(id, source, filename, account_id, started_at) \
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![
            &id,
            source.as_db(),
            filename,
            account_id,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(id)
}

/// Mark an import finished with row counts and optional error.
pub fn finish(
    conn: &Connection,
    id: &str,
    rows_imported: u32,
    rows_skipped_duplicates: u32,
    error: Option<&str>,
) -> CoreResult<()> {
    conn.execute(
        "UPDATE imports SET finished_at = ?1, rows_imported = ?2, \
              rows_skipped_duplicates = ?3, error = ?4 \
         WHERE id = ?5",
        params![
            Utc::now().to_rfc3339(),
            rows_imported as i64,
            rows_skipped_duplicates as i64,
            error,
            id
        ],
    )?;
    Ok(())
}

/// Return imports whose finished_at is NULL — surfaced as a recovery banner.
pub fn list_unfinished(conn: &Connection) -> CoreResult<Vec<Import>> {
    let mut stmt = conn.prepare(
        "SELECT id, source, filename, account_id, started_at, finished_at, \
                rows_imported, rows_skipped_duplicates, error \
         FROM imports WHERE finished_at IS NULL ORDER BY started_at DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Import {
            id: r.get(0)?,
            source: ImportSource::from_db(&r.get::<_, String>(1)?),
            filename: r.get(2)?,
            account_id: r.get(3)?,
            started_at: parse_rfc3339(&r.get::<_, String>(4)?),
            finished_at: r.get::<_, Option<String>>(5)?.as_deref().map(parse_rfc3339),
            rows_imported: r.get(6)?,
            rows_skipped_duplicates: r.get(7)?,
            error: r.get(8)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn parse_rfc3339(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .unwrap_or_else(|_| Utc::now().into())
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh(dir: &TempDir) -> Db {
        let path = dir.path().join("imp.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        db
    }

    #[test]
    fn start_then_finish_round_trip() {
        let dir = TempDir::new().unwrap();
        let db = fresh(&dir);
        let conn = db.get().unwrap();
        let id = start(&conn, ImportSource::Csv, Some("chase.csv"), None).unwrap();
        assert!(list_unfinished(&conn).unwrap().iter().any(|i| i.id == id));
        finish(&conn, &id, 42, 3, None).unwrap();
        assert!(list_unfinished(&conn).unwrap().is_empty());
    }

    #[test]
    fn finish_with_error_records_message() {
        let dir = TempDir::new().unwrap();
        let db = fresh(&dir);
        let conn = db.get().unwrap();
        let id = start(&conn, ImportSource::Csv, Some("bad.csv"), None).unwrap();
        finish(&conn, &id, 0, 0, Some("file not utf8")).unwrap();
        let row: (i64, i64, Option<String>) = conn
            .query_row(
                "SELECT rows_imported, rows_skipped_duplicates, error FROM imports WHERE id = ?1",
                params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, (0, 0, Some("file not utf8".to_string())));
    }
}
