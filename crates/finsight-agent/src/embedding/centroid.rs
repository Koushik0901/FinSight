//! Prototype (centroid) category matching — issue #92, Slice 4b.
//!
//! Each category gets ONE vector: the mean of its curated examples
//! (`category_examples`, V062) in the encoder's space. Categorizing a
//! description is then a linear cosine scan over ~tens of those vectors.
//!
//! That is deliberately *not* a nearest-neighbour search over the ledger. Epic
//! #74's framing implied "index = greenfield infra"; for centroid matching it
//! isn't — a few dozen dot products over 384 floats is microseconds, and no
//! ANN structure earns its complexity here. Only transaction-level kNN over
//! every transaction would need one, and that is out of scope.
//!
//! # What this module will never do
//!
//! Write `transactions.category_id`. Per #74's conclusion, a semantic match is
//! a *proposal* — it routes into the review lane (`category_proposals`, #87)
//! and waits for a human. The precision gate that would justify auto-applying
//! (≥98%, merchant-disjoint) is unfalsifiable until issue #89's real labeled
//! corpus exists, and writing unvalidated ML guesses into the canonical column
//! that budgets, reports and metrics all read is exactly the failure #74 was
//! scoped to prevent.

use anyhow::Result;
use finsight_core::repos::{category_centroids, category_examples};
use finsight_core::Db;
use std::collections::BTreeMap;

use super::SentenceEncoder;

/// Scales `v` to unit length. Returns `false` (leaving `v` untouched) when the
/// norm is zero or non-finite — a vector that cannot be normalized must not be
/// silently turned into something scorable.
pub fn normalize(v: &mut [f32]) -> bool {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !norm.is_finite() || norm <= f32::EPSILON {
        return false;
    }
    for x in v.iter_mut() {
        *x /= norm;
    }
    true
}

/// The prototype vector for a set of example embeddings: normalize each, take
/// the mean, normalize again.
///
/// **Normalizing BEFORE averaging is the load-bearing part.** Encoders do not
/// return equal-magnitude vectors, so a plain mean lets whichever example
/// happens to have the largest magnitude pull the prototype toward itself —
/// weighting examples by an artifact of the encoder rather than by intent.
/// Every example should count once. The final re-normalization is what lets
/// the read path treat cosine as a plain dot product.
///
/// Returns `None` for an empty input, or when every input was degenerate. A
/// category with no usable examples must end up with NO centroid — a zero
/// vector would cosine-match everything equally and quietly become a
/// catch-all.
pub fn centroid_of(vectors: &[Vec<f32>]) -> Option<Vec<f32>> {
    let dims = vectors.first()?.len();
    if dims == 0 {
        return None;
    }

    let mut sum = vec![0.0f32; dims];
    let mut counted = 0usize;
    for v in vectors {
        // A ragged batch means the encoder broke its own contract; skip rather
        // than panic on the index, and let `counted` reflect reality.
        if v.len() != dims {
            continue;
        }
        let mut unit = v.clone();
        if !normalize(&mut unit) {
            continue;
        }
        for (s, u) in sum.iter_mut().zip(unit.iter()) {
            *s += u;
        }
        counted += 1;
    }
    if counted == 0 {
        return None;
    }
    for s in sum.iter_mut() {
        *s /= counted as f32;
    }
    // Antipodal examples can cancel to ~zero. That is a genuinely undefined
    // prototype, not a usable one.
    if !normalize(&mut sum) {
        return None;
    }
    Some(sum)
}

/// Cosine similarity. Both inputs must already be unit vectors — which is why
/// this is a plain dot product — so the result is in [-1, 1].
///
/// Returns 0.0 on a length mismatch rather than panicking: the read path
/// filters by `dims` in SQL, so a mismatch here means something upstream is
/// already wrong and the safe answer is "no similarity".
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// One category's similarity to a query.
#[derive(Debug, Clone, PartialEq)]
pub struct CentroidMatch {
    pub category_id: String,
    /// Cosine similarity in [-1, 1]. NOT a probability, and deliberately not
    /// rescaled into one: mapping raw cosine onto a 0-1 "confidence" would
    /// invent a calibration that issue #93 has not measured yet.
    pub score: f32,
}

/// Rank `centroids` against a query embedding, best first.
pub fn rank(
    query: &[f32],
    centroids: &[category_centroids::CategoryCentroid],
) -> Vec<CentroidMatch> {
    let mut out: Vec<CentroidMatch> = centroids
        .iter()
        .map(|c| CentroidMatch {
            category_id: c.category_id.clone(),
            score: cosine(query, &c.vector),
        })
        .collect();
    // Descending by score; ties broken by category id so the ordering is
    // deterministic across runs (a HashMap-ordered tie would make the "top
    // match" wobble between identical inputs).
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.category_id.cmp(&b.category_id))
    });
    out
}

/// Recompute the centroid of every active category from its stored examples.
///
/// Call this after examples change. It is a full rebuild rather than an
/// incremental update on purpose: an incremental mean would need the previous
/// example count and vector to stay exactly in sync with the example table
/// through adds, removes and edits, and drift there is silent. Categories are
/// tens of rows and examples are a handful each, so a rebuild is cheap enough
/// that the simpler, always-correct option wins.
///
/// Returns how many categories now have a centroid.
///
/// The read/embed/write split is forced by the runtime: `embed` is async, and
/// DB access must happen on a blocking thread, so this reads texts, drops the
/// connection, embeds, then reopens to write. Trying to await inside the
/// blocking closure is the first thing that will fight anyone editing this.
pub async fn rebuild_all(db: &Db, encoder: &dyn SentenceEncoder) -> Result<usize> {
    // 1. Read (blocking): example texts grouped by category.
    let by_category: BTreeMap<String, Vec<String>> = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let rows = category_examples::list_for_active_categories(&mut conn)?;
            let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for e in rows {
                map.entry(e.category_id).or_default().push(e.example_text);
            }
            Ok::<_, anyhow::Error>(map)
        })
        .await??
    };

    // 2. Embed (async). One flat batch across every category rather than a
    //    call per category: the encoder shares padding and does a single
    //    forward pass, so N separate calls is strictly more work.
    let mut flat: Vec<String> = Vec::new();
    let mut spans: Vec<(String, usize, usize)> = Vec::new();
    for (category_id, texts) in &by_category {
        let start = flat.len();
        flat.extend(texts.iter().cloned());
        spans.push((category_id.clone(), start, flat.len()));
    }
    if flat.is_empty() {
        return Ok(0);
    }
    let vectors = encoder.embed(&flat).await?;

    let mut centroids: Vec<(String, Vec<f32>, i64)> = Vec::new();
    for (category_id, start, end) in spans {
        let slice: Vec<Vec<f32>> = vectors[start..end].to_vec();
        let count = slice.len() as i64;
        if let Some(c) = centroid_of(&slice) {
            centroids.push((category_id, c, count));
        }
        // No usable centroid → deliberately nothing written, so the category
        // simply does not participate in matching.
    }

    // 3. Write (blocking).
    let model_id = encoder.model_id().to_string();
    let written = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let mut n = 0usize;
            for (category_id, vector, count) in &centroids {
                category_centroids::upsert(&mut conn, category_id, &model_id, vector, *count)?;
                n += 1;
            }
            // Vectors from a previous encoder are already invisible to the
            // read path (it filters on model_id); this reclaims their space.
            category_centroids::delete_stale_models(&mut conn, &model_id)?;
            Ok::<_, anyhow::Error>(n)
        })
        .await??
    };
    Ok(written)
}

/// Cosine floor below which a match is not worth proposing at all.
///
/// This is a NOISE FLOOR, not a calibrated decision boundary. Issue #93 is the
/// slice that derives a real threshold from #88's measured precision/coverage
/// curve; until that exists any constant here is a guess, so this one is set
/// only to stop obviously-unrelated matches from filling the review queue —
/// deliberately low enough that it is not doing hidden precision work that
/// #93's calibration would then be measuring around.
pub const MIN_PROPOSAL_SCORE: f32 = 0.35;

/// How many alternatives to record alongside the top match, for the review UI
/// and for anyone later asking "what else did it consider?".
const CANDIDATES_KEPT: usize = 3;

/// Propose categories for uncategorized transactions by cosine similarity to
/// each category's prototype.
///
/// # This never writes `transactions.category_id`
///
/// Every match lands in `category_proposals` with `applied = 0` — the
/// "held-back" half of the axis V061 was built to carry. Per epic #74's
/// conclusion, an ML pass may not touch the canonical column that budgets,
/// spending, reports and metrics all read until the ≥98% merchant-disjoint
/// precision gate is actually falsifiable, which needs issue #89's real
/// labeled corpus. `proposals_never_touch_canonical` is the regression test.
///
/// Returns the number of proposals written.
pub async fn propose_for_uncategorized(db: &Db, encoder: &dyn SentenceEncoder) -> Result<usize> {
    // 1. Read (blocking). `load_uncategorized_for_proposals` reuses the SAME
    //    predicate the LLM pass uses, so the deterministic invariants (never
    //    transfers, never investment rows, never an already-categorized row)
    //    cannot drift between the two passes.
    let (rows, centroids) = {
        let db = db.clone();
        let model_id = encoder.model_id().to_string();
        let dims = encoder.dims();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let rows = crate::categorizer::load_uncategorized_for_proposals(&mut conn)?;
            let centroids = category_centroids::load_active_for_model(&mut conn, &model_id, dims)?;
            Ok::<_, anyhow::Error>((rows, centroids))
        })
        .await??
    };
    // No prototypes (no examples curated yet, or every stored vector is from a
    // different encoder) means nothing to compare against — not "propose
    // whatever is least dissimilar".
    if rows.is_empty() || centroids.is_empty() {
        return Ok(0);
    }

    // 2. Embed (async), one batch.
    let texts: Vec<String> = rows
        .iter()
        .map(|(_, merchant, _)| merchant.clone())
        .collect();
    let query_vectors = encoder.embed(&texts).await?;

    let mut proposals: Vec<(String, String, f32, String)> = Vec::new();
    for ((txn_id, merchant, _), mut qv) in rows.into_iter().zip(query_vectors) {
        // The stored centroids are unit vectors, so the query must be one too
        // or `cosine`'s dot product is not a cosine at all.
        if !normalize(&mut qv) {
            continue;
        }
        let ranked = rank(&qv, &centroids);
        let Some(best) = ranked.first() else { continue };
        if best.score < MIN_PROPOSAL_SCORE {
            continue;
        }
        let candidates = serde_json::to_string(
            &ranked
                .iter()
                .take(CANDIDATES_KEPT)
                .map(|m| serde_json::json!({ "categoryId": m.category_id, "score": m.score }))
                .collect::<Vec<_>>(),
        )?;
        proposals.push((txn_id, best.category_id.clone(), best.score, candidates));
        let _ = merchant;
    }

    // 3. Write (blocking).
    let model_id = encoder.model_id().to_string();
    let written = {
        let db = db.clone();
        tokio::task::spawn_blocking(move || {
            let mut conn = db.get()?;
            let mut n = 0usize;
            for (txn_id, category_id, score, candidates_json) in &proposals {
                finsight_core::repos::category_proposals::upsert(
                    &mut conn,
                    finsight_core::models::NewCategoryProposal {
                        txn_id: txn_id.clone(),
                        proposed_category_id: category_id.clone(),
                        // V061 reserves 'ml' for exactly this pass. The encoder
                        // id goes in `model`, which is what distinguishes
                        // centroid matching from a future reranker/SetFit pass
                        // (#95) without inventing an unreviewed enum value.
                        source: "ml".to_string(),
                        // Raw cosine, NOT rescaled into a 0-1 "confidence".
                        // Mapping it would invent a calibration #93 has not
                        // measured, and the review UI would then be showing a
                        // number nobody derived.
                        confidence: f64::from(*score),
                        rationale: None,
                        candidates_json: Some(candidates_json.clone()),
                        status: "pending".to_string(),
                        // THE line that keeps this out of the canonical column.
                        applied: false,
                        model: Some(model_id.clone()),
                    },
                )?;
                n += 1;
            }
            Ok::<_, anyhow::Error>(n)
        })
        .await??
    };
    Ok(written)
}

#[cfg(test)]
pub(crate) mod testing {
    use super::*;
    use async_trait::async_trait;

    /// A deterministic offline stand-in for the real encoder.
    ///
    /// The shipped `MiniLmEncoder` downloads ~90MB from HuggingFace on first
    /// use. Unit tests must not depend on that: it would make them slow,
    /// network-dependent, and would fail in exactly the sandboxed environments
    /// where this repo's `samples/`-dependent tests already fail. The trait is
    /// the seam that makes this substitution free.
    ///
    /// Vectors are derived from the text's bytes, so "similar" strings are not
    /// meaningfully similar here — these tests assert PLUMBING (does a centroid
    /// get written, is a stale model skipped, does an empty category abstain),
    /// never semantic quality. Semantic quality is what the eval harness
    /// measures, with the real model.
    pub struct StubEncoder {
        pub model_id: String,
        pub dims: usize,
    }

    impl StubEncoder {
        pub fn new(model_id: &str, dims: usize) -> Self {
            Self {
                model_id: model_id.to_string(),
                dims,
            }
        }
    }

    #[async_trait]
    impl SentenceEncoder for StubEncoder {
        fn model_id(&self) -> &str {
            &self.model_id
        }
        fn dims(&self) -> usize {
            self.dims
        }
        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
            Ok(texts
                .iter()
                .map(|t| {
                    let mut v = vec![0.0f32; self.dims];
                    for (i, b) in t.bytes().enumerate() {
                        v[i % self.dims] += f32::from(b) / 255.0;
                    }
                    // Guarantee a non-degenerate vector even for an empty
                    // string, so the stub never accidentally exercises the
                    // "unnormalizable" path a test didn't ask for.
                    v[0] += 1.0;
                    v
                })
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testing::StubEncoder;
    use super::*;

    #[test]
    fn normalize_refuses_a_zero_vector() {
        let mut v = vec![0.0, 0.0, 0.0];
        assert!(
            !normalize(&mut v),
            "a zero vector has no direction to preserve"
        );
        assert_eq!(
            v,
            vec![0.0, 0.0, 0.0],
            "a refused normalize must not mutate"
        );
    }

    #[test]
    fn normalize_produces_unit_length() {
        let mut v = vec![3.0, 4.0];
        assert!(normalize(&mut v));
        assert!((v.iter().map(|x| x * x).sum::<f32>().sqrt() - 1.0).abs() < 1e-6);
    }

    /// The reason `centroid_of` normalizes before averaging. A plain mean lets
    /// a large-magnitude example drag the prototype toward itself, weighting
    /// examples by an encoder artifact instead of counting each once.
    #[test]
    fn a_long_vector_does_not_outvote_a_short_one() {
        // Same two directions, wildly different magnitudes.
        let c = centroid_of(&[vec![100.0, 0.0], vec![0.0, 0.001]]).unwrap();
        assert!(
            (c[0] - c[1]).abs() < 1e-5,
            "each example should contribute equally, got {c:?}"
        );
    }

    #[test]
    fn no_examples_means_no_centroid() {
        assert!(
            centroid_of(&[]).is_none(),
            "a zero vector would match everything"
        );
    }

    #[test]
    fn antipodal_examples_cancel_to_no_centroid() {
        assert!(
            centroid_of(&[vec![1.0, 0.0], vec![-1.0, 0.0]]).is_none(),
            "a cancelled mean is an undefined prototype, not a usable one"
        );
    }

    #[test]
    fn cosine_of_identical_unit_vectors_is_one() {
        let mut a = vec![1.0, 2.0, 3.0];
        normalize(&mut a);
        assert!((cosine(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_of_mismatched_lengths_is_zero_not_a_panic() {
        assert_eq!(cosine(&[1.0, 0.0], &[1.0]), 0.0);
    }

    #[test]
    fn ranking_is_deterministic_on_ties() {
        use finsight_core::repos::category_centroids::CategoryCentroid;
        let cs = vec![
            CategoryCentroid {
                category_id: "zeta".into(),
                vector: vec![1.0, 0.0],
                example_count: 1,
            },
            CategoryCentroid {
                category_id: "alpha".into(),
                vector: vec![1.0, 0.0],
                example_count: 1,
            },
        ];
        let ranked = rank(&[1.0, 0.0], &cs);
        assert_eq!(
            ranked[0].category_id, "alpha",
            "identical scores must break ties by id, or the top match wobbles run to run"
        );
    }

    fn fresh_db() -> (tempfile::TempDir, finsight_core::Db) {
        let dir = tempfile::TempDir::new().unwrap();
        let key = finsight_core::keychain::generate_random_key();
        let db = finsight_core::Db::open(&dir.path().join("c.sqlcipher"), &key).unwrap();
        finsight_core::db::run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed(conn: &mut rusqlite::Connection, category: &str, examples: &[&str]) {
        conn.execute(
            "INSERT OR IGNORE INTO category_groups(id,label,sort_order) VALUES('g1','G',0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT OR IGNORE INTO categories(id,group_id,label,color,sort_order) VALUES(?1,'g1',?1,'#fff',0)",
            rusqlite::params![category],
        )
        .unwrap();
        for text in examples {
            category_examples::add(conn, category, text, None).unwrap();
        }
    }

    #[tokio::test]
    async fn rebuild_writes_one_centroid_per_category_with_examples() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn, "groceries", &["WHOLE FOODS", "TRADER JOES"]);
            seed(&mut conn, "dining", &["CHIPOTLE"]);
            seed(&mut conn, "travel", &[]); // no examples on purpose
        }

        let encoder = StubEncoder::new("stub-v1", 8);
        assert_eq!(rebuild_all(&db, &encoder).await.unwrap(), 2);

        let mut conn = db.get().unwrap();
        let stored = category_centroids::load_active_for_model(&mut conn, "stub-v1", 8).unwrap();
        let ids: Vec<&str> = stored.iter().map(|c| c.category_id.as_str()).collect();
        assert_eq!(
            ids,
            vec!["dining", "groceries"],
            "a category with no examples must get NO centroid rather than a catch-all vector"
        );
        assert_eq!(
            stored
                .iter()
                .find(|c| c.category_id == "groceries")
                .unwrap()
                .example_count,
            2
        );
        // Stored normalized, so the read path's dot-product-as-cosine holds.
        for c in &stored {
            let norm = c.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!(
                (norm - 1.0).abs() < 1e-5,
                "stored centroids must be unit length"
            );
        }
    }

    /// Swapping the encoder must not leave the old vectors comparable. They are
    /// skipped on read (filtered by `model_id`) AND reclaimed on rebuild.
    #[tokio::test]
    async fn changing_the_encoder_invalidates_every_stale_vector() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn, "groceries", &["WHOLE FOODS"]);
        }

        rebuild_all(&db, &StubEncoder::new("stub-v1", 8))
            .await
            .unwrap();
        {
            let mut conn = db.get().unwrap();
            assert_eq!(
                category_centroids::load_active_for_model(&mut conn, "stub-v1", 8)
                    .unwrap()
                    .len(),
                1
            );
        }

        // A different model, and a different width.
        rebuild_all(&db, &StubEncoder::new("stub-v2", 16))
            .await
            .unwrap();
        let mut conn = db.get().unwrap();
        assert!(
            category_centroids::load_active_for_model(&mut conn, "stub-v1", 8)
                .unwrap()
                .is_empty(),
            "vectors from the previous encoder must not survive as scorable rows"
        );
        assert_eq!(
            category_centroids::load_active_for_model(&mut conn, "stub-v2", 16)
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn rebuild_on_an_empty_ledger_is_a_no_op() {
        let (_d, db) = fresh_db();
        let encoder = StubEncoder::new("stub-v1", 8);
        assert_eq!(rebuild_all(&db, &encoder).await.unwrap(), 0);
    }

    fn seed_account(conn: &mut rusqlite::Connection) {
        conn.execute(
            "INSERT OR IGNORE INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Checking','Ch','USD','#fff','manual','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }

    fn insert_txn(conn: &mut rusqlite::Connection, id: &str, merchant: &str, is_transfer: bool) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,is_transfer,created_at) \
             VALUES(?1,'a1','2024-02-01T00:00:00Z',-2500,?2,'cleared',0,?3,'2024-02-01T00:00:00Z')",
            rusqlite::params![id, merchant, is_transfer as i64],
        )
        .unwrap();
    }

    /// THE regression test for this slice.
    ///
    /// Epic #74's whole reason for existing is that an unvalidated ML guess must
    /// never reach `transactions.category_id` — the column budgets, spending,
    /// reports and metrics all read. A semantic match is a proposal and nothing
    /// more until issue #89's real corpus makes the precision gate falsifiable.
    #[tokio::test]
    async fn proposals_never_touch_canonical() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn, "groceries", &["WHOLE FOODS MARKET"]);
            seed_account(&mut conn);
            insert_txn(&mut conn, "t1", "WHOLE FOODS MARKET 123", false);
        }
        let encoder = StubEncoder::new("stub-v1", 8);
        rebuild_all(&db, &encoder).await.unwrap();

        let written = propose_for_uncategorized(&db, &encoder).await.unwrap();
        assert_eq!(written, 1, "the transaction should have drawn a proposal");

        let conn = db.get().unwrap();
        let canonical: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id='t1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            canonical.is_none(),
            "a semantic match must leave category_id NULL — writing it would put an \
             unvalidated ML guess into the column every money number reads"
        );

        let (source, applied, model): (String, i64, Option<String>) = conn
            .query_row(
                "SELECT source, applied, model FROM category_proposals WHERE txn_id='t1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(source, "ml", "V061 reserves 'ml' for this pass");
        assert_eq!(applied, 0, "applied=0 is the held-back half of V061's axis");
        assert_eq!(
            model.as_deref(),
            Some("stub-v1"),
            "the encoder must be recorded"
        );
    }

    /// The semantic pass is bound by the same deterministic invariants as the
    /// LLM pass — it shares `load_uncategorized`'s predicate precisely so these
    /// cannot drift apart.
    #[tokio::test]
    async fn transfers_and_already_categorized_rows_are_never_proposed_for() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn, "groceries", &["WHOLE FOODS MARKET"]);
            seed_account(&mut conn);
            insert_txn(&mut conn, "t_transfer", "TRANSFER TO SAVINGS", true);
            insert_txn(&mut conn, "t_done", "WHOLE FOODS MARKET", false);
            conn.execute(
                "UPDATE transactions SET category_id='groceries' WHERE id='t_done'",
                [],
            )
            .unwrap();
        }
        let encoder = StubEncoder::new("stub-v1", 8);
        rebuild_all(&db, &encoder).await.unwrap();

        assert_eq!(
            propose_for_uncategorized(&db, &encoder).await.unwrap(),
            0,
            "a transfer is not spending, and an already-categorized row is not up for grabs"
        );
        let conn = db.get().unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM category_proposals", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    /// With no prototypes to compare against, the answer is "no opinion" — not
    /// "whichever category is least dissimilar".
    #[tokio::test]
    async fn no_centroids_means_no_proposals() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn, "groceries", &[]); // category exists, no examples
            seed_account(&mut conn);
            insert_txn(&mut conn, "t1", "MYSTERY MERCHANT", false);
        }
        let encoder = StubEncoder::new("stub-v1", 8);
        rebuild_all(&db, &encoder).await.unwrap();
        assert_eq!(propose_for_uncategorized(&db, &encoder).await.unwrap(), 0);
    }

    /// Centroids written by a previous encoder must not be scored against a new
    /// encoder's query vectors — the comparison would return a plausible number
    /// from two unrelated spaces.
    #[tokio::test]
    async fn a_stale_encoder_yields_no_proposals_rather_than_wrong_ones() {
        let (_d, db) = fresh_db();
        {
            let mut conn = db.get().unwrap();
            seed(&mut conn, "groceries", &["WHOLE FOODS MARKET"]);
            seed_account(&mut conn);
            insert_txn(&mut conn, "t1", "WHOLE FOODS MARKET 123", false);
        }
        // Build prototypes with one encoder...
        rebuild_all(&db, &StubEncoder::new("stub-v1", 8))
            .await
            .unwrap();
        // ...then score with a different one, WITHOUT rebuilding.
        let written = propose_for_uncategorized(&db, &StubEncoder::new("stub-v2", 8))
            .await
            .unwrap();
        assert_eq!(
            written, 0,
            "stale vectors must be skipped, not compared across embedding spaces"
        );
    }
}
