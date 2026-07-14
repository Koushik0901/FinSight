//! CsvImportMapping — describes how a particular CSV's columns map to
//! NewTransaction fields. Persisted per-account in csv_import_mappings.

use crate::error::{ProviderError, ProviderResult};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AmountConvention {
    NegativeIsOutflow,
    PositiveIsOutflow,
    SplitDebitCredit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub enum ColumnRole {
    Date,
    Amount,
    Merchant,
    Notes,
    Category,
    Skip,
    Debit,
    Credit,
    // Investment-CSV roles (brokerage exports like Wealthsimple). Serialized
    // by variant name, so mappings saved before these existed still decode.
    ActivityType,
    ActivitySubType,
    Symbol,
    SecurityName,
    Quantity,
    UnitPrice,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct CsvImportMapping {
    pub skip_header_rows: u32,
    pub columns: Vec<ColumnRole>,
    pub date_format: String,
    pub amount_convention: AmountConvention,
    #[serde(default = "default_decimal")]
    pub decimal_separator: char,
    #[serde(default)]
    pub delimiter: Option<char>,
}

fn default_decimal() -> char {
    '.'
}

pub fn load(conn: &Connection, account_id: &str) -> ProviderResult<Option<CsvImportMapping>> {
    let row: Option<String> = conn
        .query_row(
            "SELECT mapping_json FROM csv_import_mappings WHERE account_id = ?1",
            params![account_id],
            |r| r.get(0),
        )
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
        .map_err(|e| ProviderError::Internal(format!("load mapping: {e}")))?;
    match row {
        None => Ok(None),
        Some(json) => serde_json::from_str(&json)
            .map(Some)
            .map_err(|e| ProviderError::InvalidMapping(format!("decode: {e}"))),
    }
}

pub fn save(conn: &Connection, account_id: &str, mapping: &CsvImportMapping) -> ProviderResult<()> {
    let json = serde_json::to_string(mapping)
        .map_err(|e| ProviderError::InvalidMapping(format!("encode: {e}")))?;
    conn.execute(
        "INSERT INTO csv_import_mappings(account_id, mapping_json, last_used_at) \
         VALUES(?1, ?2, ?3) \
         ON CONFLICT(account_id) DO UPDATE SET \
            mapping_json = excluded.mapping_json, \
            last_used_at = excluded.last_used_at",
        params![account_id, json, Utc::now().to_rfc3339()],
    )
    .map_err(|e| ProviderError::Internal(format!("save mapping: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use finsight_core::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db_with_account() -> (TempDir, Db, String) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("m.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        let acct_id = uuid::Uuid::new_v4().to_string();
        db.get().unwrap().execute(
            "INSERT INTO accounts(id, owner, bank, type, name, currency, color, created_at, source) \
             VALUES(?1, 'joint', 'Chase', 'Checking', 'Test', 'USD', '#000', ?2, 'manual')",
            params![&acct_id, Utc::now().to_rfc3339()],
        ).unwrap();
        (dir, db, acct_id)
    }

    fn sample_mapping() -> CsvImportMapping {
        CsvImportMapping {
            skip_header_rows: 1,
            columns: vec![
                ColumnRole::Skip,
                ColumnRole::Date,
                ColumnRole::Merchant,
                ColumnRole::Amount,
            ],
            date_format: "%m/%d/%Y".to_string(),
            amount_convention: AmountConvention::NegativeIsOutflow,
            decimal_separator: '.',
            delimiter: None,
        }
    }

    #[test]
    fn load_returns_none_for_unknown_account() {
        let (_d, db, acct) = fresh_db_with_account();
        let conn = db.get().unwrap();
        assert!(load(&conn, &acct).unwrap().is_none());
    }

    #[test]
    fn round_trip_save_then_load() {
        let (_d, db, acct) = fresh_db_with_account();
        let conn = db.get().unwrap();
        save(&conn, &acct, &sample_mapping()).unwrap();
        let got = load(&conn, &acct).unwrap().unwrap();
        assert_eq!(got.date_format, "%m/%d/%Y");
        assert_eq!(got.skip_header_rows, 1);
    }

    #[test]
    fn save_twice_overwrites() {
        let (_d, db, acct) = fresh_db_with_account();
        let conn = db.get().unwrap();
        let mut m = sample_mapping();
        save(&conn, &acct, &m).unwrap();
        m.skip_header_rows = 5;
        save(&conn, &acct, &m).unwrap();
        let got = load(&conn, &acct).unwrap().unwrap();
        assert_eq!(got.skip_header_rows, 5);
    }
}
