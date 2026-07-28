//! Per-category exemplar storage (issue #91, Slice 4a).
//!
//! Extends the V034 `categories.guidance` plumbing pattern from "one free-text
//! blob per category" to "a small set of exemplar descriptions per category",
//! keyed by the category's STABLE id so everything survives a rename.
//!
//! Nothing reads these yet — issue #92 embeds `example_text` into a
//! prototype/centroid vector per category. This module is storage + CRUD only;
//! the categorizer prompt is deliberately untouched.

use crate::error::{CoreError, CoreResult};
use crate::models::CategoryExample;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

const SELECT_COLUMNS: &str = "id, category_id, example_text, source_txn_id, created_at";

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<CategoryExample> {
    let created_at_s: String = r.get(4)?;
    Ok(CategoryExample {
        id: r.get(0)?,
        category_id: r.get(1)?,
        example_text: r.get(2)?,
        source_txn_id: r.get(3)?,
        created_at: DateTime::parse_from_rfc3339(&created_at_s)
            .map(|d| d.with_timezone(&Utc))
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
    })
}

/// Add an exemplar to a category.
///
/// `example_text` is trimmed and must be non-empty — the same validation
/// `set_guidance` applies (trim + drop empty) and `create`/`rename` apply to a
/// label. The category must exist; a typo'd id is a caller bug, not a silent
/// no-op, because the row would otherwise be unreachable through
/// `list_for_category`.
///
/// Adding an example that already exists for this category is IDEMPOTENT: the
/// existing row is returned unchanged rather than raising a constraint error.
/// A duplicate would double-weight that point in #92's centroid mean, and
/// "click the same transaction twice" is a user slip, not an error worth a
/// toast.
pub fn add(
    conn: &mut Connection,
    category_id: &str,
    example_text: &str,
    source_txn_id: Option<&str>,
) -> CoreResult<CategoryExample> {
    let text = example_text.trim();
    if text.is_empty() {
        return Err(CoreError::InvalidState(
            "example text must not be empty".into(),
        ));
    }

    let category_exists: bool = conn
        .query_row("SELECT 1 FROM categories WHERE id = ?1", [category_id], |_| {
            Ok(true)
        })
        .optional()?
        .unwrap_or(false);
    if !category_exists {
        return Err(CoreError::InvalidState("category not found".into()));
    }

    // Idempotent add: return the row that's already there.
    if let Some(existing) = conn
        .query_row(
            &format!(
                "SELECT {SELECT_COLUMNS} FROM category_examples \
                 WHERE category_id = ?1 AND example_text = ?2"
            ),
            params![category_id, text],
            map_row,
        )
        .optional()?
    {
        return Ok(existing);
    }

    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO category_examples(id, category_id, example_text, source_txn_id, created_at) \
         VALUES(?1, ?2, ?3, ?4, ?5)",
        params![id, category_id, text, source_txn_id, now],
    )?;

    Ok(CategoryExample {
        id,
        category_id: category_id.to_string(),
        example_text: text.to_string(),
        source_txn_id: source_txn_id.map(str::to_string),
        created_at: DateTime::parse_from_rfc3339(&now)
            .expect("rfc3339 round-trip")
            .with_timezone(&Utc),
    })
}

/// Remove one exemplar by its own id. No-op if the id does not exist, matching
/// `update_color` / `rename`, which are likewise no-ops for a missing row.
pub fn remove(conn: &mut Connection, id: &str) -> CoreResult<()> {
    conn.execute("DELETE FROM category_examples WHERE id = ?1", [id])?;
    Ok(())
}

/// Every exemplar for a category, oldest first.
///
/// Deliberately does NOT filter on `categories.archived_at`: this mirrors
/// `categories::list`, which still returns `guidance` on archived rows. See
/// `list_for_active_categories` for the active-only view.
pub fn list_for_category(
    conn: &mut Connection,
    category_id: &str,
) -> CoreResult<Vec<CategoryExample>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM category_examples \
         WHERE category_id = ?1 ORDER BY created_at, id"
    ))?;
    let rows = stmt.query_map([category_id], map_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Every exemplar belonging to a NON-archived category.
///
/// The active-only counterpart to `categories::guidance_hints`, which likewise
/// filters `archived_at IS NULL`. Archiving therefore hides examples from
/// anything that consumes them (nothing does yet — #92 will) without deleting
/// them: the storage/consumer split is exactly what `guidance` already does.
pub fn list_for_active_categories(conn: &mut Connection) -> CoreResult<Vec<CategoryExample>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.category_id, e.example_text, e.source_txn_id, e.created_at \
         FROM category_examples e \
         JOIN categories c ON c.id = e.category_id \
         WHERE c.archived_at IS NULL \
         ORDER BY e.category_id, e.created_at, e.id",
    )?;
    let rows = stmt.query_map([], map_row)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::categories;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let (dir, db) = crate::testing::migrated_db();
        (dir, db)
    }

    fn seed_group(conn: &mut Connection) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('daily', 'Daily', 0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn add_list_and_remove_round_trip() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = categories::create(&mut conn, "Coffee Shops", Some("daily"), "#111").unwrap();

        let a = add(&mut conn, &cat.id, "SQ *BLUE BOTTLE COFFEE", None).unwrap();
        let b = add(&mut conn, &cat.id, "STARBUCKS #4021", None).unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(a.source_txn_id, None);

        let listed = list_for_category(&mut conn, &cat.id).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().any(|e| e.example_text == "SQ *BLUE BOTTLE COFFEE"));

        remove(&mut conn, &a.id).unwrap();
        let listed = list_for_category(&mut conn, &cat.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, b.id);
    }

    #[test]
    fn add_trims_and_rejects_empty_text() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = categories::create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();

        let e = add(&mut conn, &cat.id, "  TIM HORTONS  ", None).unwrap();
        assert_eq!(e.example_text, "TIM HORTONS");

        assert!(add(&mut conn, &cat.id, "   ", None).is_err());
        assert!(add(&mut conn, &cat.id, "", None).is_err());
    }

    #[test]
    fn add_rejects_unknown_category() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        assert!(add(&mut conn, "does-not-exist", "WHATEVER", None).is_err());
    }

    #[test]
    fn add_is_idempotent_per_category_and_text() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let coffee = categories::create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        let travel = categories::create(&mut conn, "Travel", Some("daily"), "#222").unwrap();

        let first = add(&mut conn, &coffee.id, "STARBUCKS #4021", None).unwrap();
        // Same text, same category (and untrimmed) -> the SAME row back, no error.
        let again = add(&mut conn, &coffee.id, " STARBUCKS #4021 ", None).unwrap();
        assert_eq!(first.id, again.id);
        assert_eq!(list_for_category(&mut conn, &coffee.id).unwrap().len(), 1);

        // The uniqueness is per (category, text) — a different category may hold
        // the same exemplar string.
        add(&mut conn, &travel.id, "STARBUCKS #4021", None).unwrap();
        assert_eq!(list_for_category(&mut conn, &travel.id).unwrap().len(), 1);
    }

    #[test]
    fn remove_is_noop_for_missing_id() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        remove(&mut conn, "no-such-example").unwrap();
    }

    /// Acceptance criterion 2: examples are keyed by the STABLE category id, so
    /// a rename (which touches `categories.label` only) cannot detach them —
    /// the same guarantee `guidance` has had since V034.
    #[test]
    fn examples_survive_a_category_rename() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = categories::create(&mut conn, "Coffee Shops", Some("daily"), "#111").unwrap();
        add(&mut conn, &cat.id, "SQ *BLUE BOTTLE COFFEE", None).unwrap();
        add(&mut conn, &cat.id, "STARBUCKS #4021", None).unwrap();

        categories::rename(&mut conn, &cat.id, "Cafés").unwrap();

        // The id is unchanged, the label is not, and the examples still resolve.
        let renamed = categories::list(&mut conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == cat.id)
            .unwrap();
        assert_eq!(renamed.label, "Cafés");

        let listed = list_for_category(&mut conn, &cat.id).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|e| e.category_id == cat.id));

        // And they're still live for the active-only accessor.
        assert_eq!(list_for_active_categories(&mut conn).unwrap().len(), 2);
    }

    /// Acceptance criterion 3: archiving must not silently orphan examples.
    ///
    /// The documented behaviour is HIDDEN-BUT-RETAINED, chosen to match what
    /// `guidance` already does: `categories::archive` never clears `guidance`,
    /// and `categories::guidance_hints` is what filters `archived_at IS NULL`.
    /// Both halves are asserted here — retained in storage AND excluded from
    /// the active-only accessor — because either half alone would not
    /// demonstrate consistency with `guidance`.
    #[test]
    fn archive_retains_examples_but_hides_them_like_guidance() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = categories::create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        categories::set_guidance(&mut conn, &cat.id, Some("Any coffee shop or café.")).unwrap();
        add(&mut conn, &cat.id, "SQ *BLUE BOTTLE COFFEE", None).unwrap();

        assert_eq!(categories::guidance_hints(&mut conn).unwrap().len(), 1);
        assert_eq!(list_for_active_categories(&mut conn).unwrap().len(), 1);

        categories::archive(&mut conn, &cat.id).unwrap();

        // Consumer-facing (active-only) views drop both, in lockstep.
        assert!(categories::guidance_hints(&mut conn).unwrap().is_empty());
        assert!(list_for_active_categories(&mut conn).unwrap().is_empty());

        // Storage retains both: guidance is still on the row...
        let archived = categories::list(&mut conn)
            .unwrap()
            .into_iter()
            .find(|c| c.id == cat.id)
            .unwrap();
        assert!(archived.archived_at.is_some());
        assert_eq!(archived.guidance.as_deref(), Some("Any coffee shop or café."));
        // ...and the examples still resolve by category id. Nothing is orphaned.
        let listed = list_for_category(&mut conn, &cat.id).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].example_text, "SQ *BLUE BOTTLE COFFEE");
    }

    /// `source_txn_id` is provenance, not a dependency: losing the source
    /// transaction (a re-import, a delete, a factory reset) must degrade the
    /// example to "hand-typed", never delete it. This is precisely why
    /// `example_text` is a denormalized snapshot rather than a join.
    #[test]
    fn deleting_the_source_transaction_keeps_the_example_and_nulls_the_link() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = categories::create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();

        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, color, created_at) \
             VALUES('acct', 'me', 'Bank', 'checking', 'Chequing', '#111', ?1)",
            [&now],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, created_at) \
             VALUES('txn-1', 'acct', '2026-01-05', -540, 'SQ *BLUE BOTTLE COFFEE', ?1)",
            [&now],
        )
        .unwrap();

        let e = add(
            &mut conn,
            &cat.id,
            "SQ *BLUE BOTTLE COFFEE",
            Some("txn-1"),
        )
        .unwrap();
        assert_eq!(e.source_txn_id.as_deref(), Some("txn-1"));

        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute("DELETE FROM transactions WHERE id = 'txn-1'", [])
            .unwrap();

        let listed = list_for_category(&mut conn, &cat.id).unwrap();
        assert_eq!(listed.len(), 1, "the example must survive its source txn");
        assert_eq!(listed[0].example_text, "SQ *BLUE BOTTLE COFFEE");
        assert_eq!(listed[0].source_txn_id, None, "the link is SET NULL");
    }

    /// A hard DELETE of the category (as opposed to archiving) is the one case
    /// where examples should go away — they have no owner and no meaning.
    #[test]
    fn hard_deleting_a_category_cascades_examples() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_group(&mut conn);
        let cat = categories::create(&mut conn, "Coffee", Some("daily"), "#111").unwrap();
        add(&mut conn, &cat.id, "STARBUCKS #4021", None).unwrap();

        conn.pragma_update(None, "foreign_keys", true).unwrap();
        conn.execute("DELETE FROM categories WHERE id = ?1", [&cat.id])
            .unwrap();

        assert!(list_for_category(&mut conn, &cat.id).unwrap().is_empty());
    }
}
