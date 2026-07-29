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
pub fn rank(query: &[f32], centroids: &[category_centroids::CategoryCentroid]) -> Vec<CentroidMatch> {
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
            Self { model_id: model_id.to_string(), dims }
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
        assert!(!normalize(&mut v), "a zero vector has no direction to preserve");
        assert_eq!(v, vec![0.0, 0.0, 0.0], "a refused normalize must not mutate");
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
        assert!(centroid_of(&[]).is_none(), "a zero vector would match everything");
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
            CategoryCentroid { category_id: "zeta".into(), vector: vec![1.0, 0.0], example_count: 1 },
            CategoryCentroid { category_id: "alpha".into(), vector: vec![1.0, 0.0], example_count: 1 },
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
            stored.iter().find(|c| c.category_id == "groceries").unwrap().example_count,
            2
        );
        // Stored normalized, so the read path's dot-product-as-cosine holds.
        for c in &stored {
            let norm = c.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5, "stored centroids must be unit length");
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

        rebuild_all(&db, &StubEncoder::new("stub-v1", 8)).await.unwrap();
        {
            let mut conn = db.get().unwrap();
            assert_eq!(
                category_centroids::load_active_for_model(&mut conn, "stub-v1", 8).unwrap().len(),
                1
            );
        }

        // A different model, and a different width.
        rebuild_all(&db, &StubEncoder::new("stub-v2", 16)).await.unwrap();
        let mut conn = db.get().unwrap();
        assert!(
            category_centroids::load_active_for_model(&mut conn, "stub-v1", 8).unwrap().is_empty(),
            "vectors from the previous encoder must not survive as scorable rows"
        );
        assert_eq!(
            category_centroids::load_active_for_model(&mut conn, "stub-v2", 16).unwrap().len(),
            1
        );
    }

    #[tokio::test]
    async fn rebuild_on_an_empty_ledger_is_a_no_op() {
        let (_d, db) = fresh_db();
        let encoder = StubEncoder::new("stub-v1", 8);
        assert_eq!(rebuild_all(&db, &encoder).await.unwrap(), 0);
    }
}
