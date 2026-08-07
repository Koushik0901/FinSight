//! Prototype (centroid) embedding storage per category — issue #92, Slice 4b.
//!
//! One L2-normalized vector per category, the mean of its curated examples
//! (`category_examples`, V062) in some encoder's space. See
//! `migrations/V063__category_centroids.sql` for why this is a side table and
//! why `model_id`/`dims` are load-bearing rather than bookkeeping.
//!
//! # The one invariant that matters
//!
//! **A vector is only ever comparable to a query vector from the same model.**
//! Cosine similarity between two different embedding spaces does not error and
//! does not produce an obviously wrong answer — it produces a *plausible* one.
//! So [`load_active_for_model`] filters on `model_id` AND `dims` in SQL, and
//! callers cannot opt out: there is deliberately no "load every centroid"
//! accessor for the scoring path to reach for by mistake.

use crate::error::{CoreError, CoreResult};
use chrono::Utc;
use rusqlite::{params, Connection};

/// A stored centroid, already decoded and known to match the requested model.
#[derive(Debug, Clone, PartialEq)]
pub struct CategoryCentroid {
    pub category_id: String,
    /// L2-normalized, so cosine similarity against another unit vector is a
    /// plain dot product.
    pub vector: Vec<f32>,
    /// How many examples went into the mean. A centroid built from one example
    /// is a point, not a prototype — callers that care can weigh accordingly.
    pub example_count: i64,
}

/// Encode `vector` as little-endian f32s.
///
/// Explicit LE rather than native-endian: a SQLCipher file is a portable
/// artifact (backups, `VACUUM INTO`, moving a self-hosted instance between
/// machines), and a big-endian reader silently reinterpreting these bytes
/// would produce garbage similarities rather than an error.
fn encode(vector: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(vector.len() * 4);
    for v in vector {
        out.extend_from_slice(&v.to_le_bytes());
    }
    out
}

fn decode(blob: &[u8], dims: i64) -> CoreResult<Vec<f32>> {
    let dims = usize::try_from(dims)
        .map_err(|_| CoreError::InvalidState("category_centroids.dims is negative".into()))?;
    // The stored `dims` and the actual blob length must agree. They can only
    // disagree through corruption or a partial write, and the safe response is
    // to refuse the row rather than reinterpret a truncated blob as a shorter
    // valid vector — which would score, and score wrongly.
    if blob.len() != dims * 4 {
        return Err(CoreError::InvalidState(format!(
            "centroid blob is {} bytes but dims={dims} implies {}",
            blob.len(),
            dims * 4
        )));
    }
    Ok(blob
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Insert or replace a category's centroid.
///
/// `vector` MUST already be L2-normalized; [`crate::repos::category_centroids`]
/// stores what it is given. Normalizing at write time (rather than at read) is
/// what lets the read path treat cosine as a dot product without re-deriving a
/// norm it has no way to verify.
pub fn upsert(
    conn: &mut Connection,
    category_id: &str,
    model_id: &str,
    vector: &[f32],
    example_count: i64,
) -> CoreResult<()> {
    conn.execute(
        "INSERT INTO category_centroids
             (category_id, model_id, dims, vector, example_count, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(category_id) DO UPDATE SET
             model_id = excluded.model_id,
             dims = excluded.dims,
             vector = excluded.vector,
             example_count = excluded.example_count,
             updated_at = excluded.updated_at",
        params![
            category_id,
            model_id,
            vector.len() as i64,
            encode(vector),
            example_count,
            Utc::now().to_rfc3339()
        ],
    )?;
    Ok(())
}

/// Every centroid that is (a) from `model_id`, (b) `dims` long, and (c) belongs
/// to a category that is not archived.
///
/// All three filters are the point:
/// - **model/dims** — see the module doc. A mismatch is skipped, never scored.
/// - **archived** — `categories::archive` is a soft delete, so the centroid row
///   survives on purpose (un-archiving shouldn't need a re-embed, which costs a
///   model load). This join is therefore what actually stops an archived
///   category from continuing to match, and it mirrors how `guidance_hints` and
///   the active-examples accessor already behave.
pub fn load_active_for_model(
    conn: &mut Connection,
    model_id: &str,
    dims: usize,
) -> CoreResult<Vec<CategoryCentroid>> {
    let mut stmt = conn.prepare(
        "SELECT cc.category_id, cc.vector, cc.dims, cc.example_count
           FROM category_centroids cc
           JOIN categories c ON c.id = cc.category_id
          WHERE cc.model_id = ?1
            AND cc.dims = ?2
            AND c.archived_at IS NULL
          ORDER BY cc.category_id",
    )?;
    let rows = stmt.query_map(params![model_id, dims as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Vec<u8>>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (category_id, blob, dims, example_count) = row?;
        out.push(CategoryCentroid {
            category_id,
            vector: decode(&blob, dims)?,
            example_count,
        });
    }
    Ok(out)
}

/// Drop one category's centroid — used when its last example is removed, since
/// a category with no examples must have NO prototype rather than a stale one.
pub fn delete_for_category(conn: &mut Connection, category_id: &str) -> CoreResult<()> {
    conn.execute(
        "DELETE FROM category_centroids WHERE category_id = ?1",
        params![category_id],
    )?;
    Ok(())
}

/// Drop every centroid NOT produced by `model_id`.
///
/// The encoder-swap path. Stale vectors are already skipped at read time, so
/// this is about reclaiming space and keeping the table honest rather than
/// about correctness — which is the right split: correctness must not depend on
/// a cleanup pass having run.
pub fn delete_stale_models(conn: &mut Connection, model_id: &str) -> CoreResult<usize> {
    Ok(conn.execute(
        "DELETE FROM category_centroids WHERE model_id <> ?1",
        params![model_id],
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::run_migrations;

    fn db() -> (tempfile::TempDir, crate::Db) {
        let dir = tempfile::TempDir::new().unwrap();
        let key = crate::keychain::generate_random_key();
        let db = crate::Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_category(conn: &mut Connection, id: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES(?1,'g1',?1,'#fff',0)",
            params![id],
        )
        .unwrap();
    }

    #[test]
    fn round_trips_a_vector_exactly() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");

        let v = vec![0.5f32, -0.25, 0.125, 0.0];
        upsert(&mut conn, "groceries", "model-a", &v, 3).unwrap();

        let got = load_active_for_model(&mut conn, "model-a", 4).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(
            got[0].vector, v,
            "f32 bytes must survive the round trip exactly"
        );
        assert_eq!(got[0].example_count, 3);
    }

    /// The invariant the whole slice rests on. A vector from another model is
    /// not a wrong answer, it is a PLAUSIBLE one — cosine between two embedding
    /// spaces returns a real-looking number. It must be skipped, not scored.
    #[test]
    fn a_vector_from_another_model_is_invisible() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");
        upsert(&mut conn, "groceries", "old-model", &[1.0, 0.0], 2).unwrap();

        assert!(
            load_active_for_model(&mut conn, "new-model", 2)
                .unwrap()
                .is_empty(),
            "a centroid from a different encoder must never reach the scorer"
        );
        // Same model, wrong dimensionality — equally incomparable.
        assert!(
            load_active_for_model(&mut conn, "old-model", 384)
                .unwrap()
                .is_empty(),
            "dims must be checked too; a same-named model can change width"
        );
    }

    #[test]
    fn an_archived_category_stops_matching_but_keeps_its_vector() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");
        upsert(&mut conn, "groceries", "m", &[1.0, 0.0], 1).unwrap();

        crate::repos::categories::archive(&mut conn, "groceries").unwrap();
        assert!(
            load_active_for_model(&mut conn, "m", 2).unwrap().is_empty(),
            "an archived category must not keep matching transactions"
        );

        // …but the row survives, so un-archiving does not require a re-embed.
        let still_there: i64 = conn
            .query_row("SELECT COUNT(*) FROM category_centroids", [], |r| r.get(0))
            .unwrap();
        assert_eq!(still_there, 1);
    }

    #[test]
    fn upsert_replaces_rather_than_accumulating() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");

        upsert(&mut conn, "groceries", "m", &[1.0, 0.0], 1).unwrap();
        upsert(&mut conn, "groceries", "m", &[0.0, 1.0], 5).unwrap();

        let got = load_active_for_model(&mut conn, "m", 2).unwrap();
        assert_eq!(got.len(), 1, "one centroid per category, always");
        assert_eq!(got[0].vector, vec![0.0, 1.0]);
        assert_eq!(got[0].example_count, 5);
    }

    #[test]
    fn deleting_a_category_takes_its_centroid_with_it() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");
        upsert(&mut conn, "groceries", "m", &[1.0, 0.0], 1).unwrap();

        conn.execute("DELETE FROM categories WHERE id='groceries'", [])
            .unwrap();

        let orphans: i64 = conn
            .query_row("SELECT COUNT(*) FROM category_centroids", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            orphans, 0,
            "ON DELETE CASCADE must not leave a matching orphan"
        );
    }

    #[test]
    fn stale_model_cleanup_keeps_only_the_current_encoder() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "a");
        seed_category(&mut conn, "b");
        upsert(&mut conn, "a", "old", &[1.0, 0.0], 1).unwrap();
        upsert(&mut conn, "b", "new", &[0.0, 1.0], 1).unwrap();

        assert_eq!(delete_stale_models(&mut conn, "new").unwrap(), 1);
        assert_eq!(load_active_for_model(&mut conn, "new", 2).unwrap().len(), 1);
    }

    /// A truncated blob must be refused, not reinterpreted as a shorter vector
    /// — a short read would still produce a scorable number.
    #[test]
    fn a_corrupt_blob_errors_instead_of_scoring() {
        let (_d, db) = db();
        let mut conn = db.get().unwrap();
        seed_category(&mut conn, "groceries");
        upsert(&mut conn, "groceries", "m", &[1.0, 0.0, 0.0, 0.0], 1).unwrap();
        conn.execute(
            "UPDATE category_centroids SET vector = ?1 WHERE category_id='groceries'",
            params![vec![0u8; 8]], // half the bytes `dims=4` promises
        )
        .unwrap();

        assert!(load_active_for_model(&mut conn, "m", 4).is_err());
    }
}
