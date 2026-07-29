//! Centroid (prototype) predictor for the #88 harness — the measurement half
//! of issue #92.
//!
//! # What is actually being measured
//!
//! Category prototypes are built from the **reference** half of the
//! merchant-disjoint split and scored against the **holdout** half. That is the
//! whole point: the holdout contains merchants the prototypes have never seen,
//! so the resulting precision speaks to generalization rather than to recall of
//! memorized strings. Building centroids from the full corpus and then scoring
//! the holdout would leak — the number would look better and mean nothing.
//!
//! # Why this module takes vectors, not an encoder
//!
//! The real encoder downloads ~90MB from HuggingFace on first use. Keeping the
//! scoring logic pure — embeddings in, predictions out — means it is unit
//! testable offline with hand-written vectors, and only the binary that wants a
//! real number pays the model cost. It also keeps the arithmetic reviewable
//! independently of whether the model is any good.
//!
//! The centroid math itself is NOT reimplemented here: it is
//! `finsight_agent::embedding::centroid`, the same code the production pass
//! uses. A harness that measured a private copy of the algorithm would be
//! measuring something the app does not run.

use super::corpus::LabeledExample;
use super::predictors::Prediction;
use finsight_agent::embedding::centroid;
use std::collections::BTreeMap;

/// A category prototype: the mean of its reference-half example embeddings.
#[derive(Debug, Clone)]
pub struct Prototype {
    pub category: String,
    /// L2-normalized, so cosine is a dot product.
    pub vector: Vec<f32>,
    pub example_count: usize,
}

/// Build one prototype per category from `reference` examples and their
/// embeddings (parallel arrays, same order).
///
/// Categories whose examples all degenerate produce no prototype rather than a
/// zero vector — a zero vector would score equally against everything and
/// quietly become a catch-all, which in a precision harness would look like
/// broad coverage instead of the bug it is.
pub fn build_prototypes(reference: &[LabeledExample], vectors: &[Vec<f32>]) -> Vec<Prototype> {
    let mut by_category: BTreeMap<&str, Vec<Vec<f32>>> = BTreeMap::new();
    for (ex, v) in reference.iter().zip(vectors.iter()) {
        by_category.entry(ex.category.as_str()).or_default().push(v.clone());
    }
    by_category
        .into_iter()
        .filter_map(|(category, vs)| {
            let count = vs.len();
            centroid::centroid_of(&vs).map(|vector| Prototype {
                category: category.to_string(),
                vector,
                example_count: count,
            })
        })
        .collect()
}

/// Predict a category for one already-embedded description.
///
/// `min_score` is the abstain floor: below it the predictor returns
/// [`Prediction::abstain`] rather than its least-bad guess. An abstain is
/// scored as "not predicted" (it lowers coverage, not precision), which is the
/// behaviour the production pass has too — it declines to propose rather than
/// filling the review queue with noise.
pub fn predict_centroid(query: &[f32], prototypes: &[Prototype], min_score: f32) -> Prediction {
    let mut best: Option<(&str, f32)> = None;
    for p in prototypes {
        let score = centroid::cosine(query, &p.vector);
        // Strictly-greater keeps the FIRST of equal scores, and `prototypes`
        // is category-ordered, so ties resolve deterministically instead of
        // wobbling between runs.
        if best.is_none_or(|(_, b)| score > b) {
            best = Some((p.category.as_str(), score));
        }
    }
    match best {
        Some((category, score)) if score >= min_score => {
            Prediction::of(category, f64::from(score))
        }
        _ => Prediction::abstain(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: &str, merchant: &str, category: &str) -> LabeledExample {
        LabeledExample {
            id: id.to_string(),
            merchant_text: merchant.to_string(),
            merchant_id: merchant.to_string(),
            category: category.to_string(),
            notes: None,
        }
    }

    #[test]
    fn prototypes_are_one_per_category_and_unit_length() {
        let reference = vec![
            ex("1", "WHOLE FOODS", "groceries"),
            ex("2", "TRADER JOES", "groceries"),
            ex("3", "CHIPOTLE", "dining"),
        ];
        let vectors = vec![vec![1.0, 0.0], vec![0.9, 0.1], vec![0.0, 1.0]];

        let protos = build_prototypes(&reference, &vectors);
        assert_eq!(protos.len(), 2);
        assert_eq!(protos[0].category, "dining");
        assert_eq!(protos[1].category, "groceries");
        assert_eq!(protos[1].example_count, 2);
        for p in &protos {
            let norm = p.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn picks_the_nearest_prototype() {
        let protos = vec![
            Prototype { category: "groceries".into(), vector: vec![1.0, 0.0], example_count: 1 },
            Prototype { category: "dining".into(), vector: vec![0.0, 1.0], example_count: 1 },
        ];
        let p = predict_centroid(&[0.95, 0.31], &protos, 0.0);
        assert_eq!(p.category.as_deref(), Some("groceries"));
    }

    /// Abstaining costs coverage, never precision. A predictor that guesses
    /// when it has no signal would inflate the very number this harness exists
    /// to measure honestly.
    #[test]
    fn abstains_below_the_floor_instead_of_guessing() {
        let protos = vec![Prototype {
            category: "groceries".into(),
            vector: vec![1.0, 0.0],
            example_count: 1,
        }];
        // Orthogonal query — cosine 0, well under the floor.
        let p = predict_centroid(&[0.0, 1.0], &protos, 0.35);
        assert!(p.category.is_none(), "no signal must mean no prediction");
    }

    #[test]
    fn no_prototypes_means_abstain() {
        assert!(predict_centroid(&[1.0, 0.0], &[], 0.0).category.is_none());
    }

    /// A category whose every example is degenerate must drop out entirely
    /// rather than contributing a zero vector that matches everything.
    #[test]
    fn a_degenerate_category_produces_no_prototype() {
        let reference = vec![ex("1", "X", "groceries")];
        let protos = build_prototypes(&reference, &[vec![0.0, 0.0]]);
        assert!(protos.is_empty());
    }
}
