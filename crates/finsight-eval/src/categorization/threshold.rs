//! Threshold sweep: precision/coverage as a function of a confidence cutoff.
//!
//! Only meaningful for sources whose confidence varies per example (e.g. an
//! `llm` or future embedding-similarity source). `builtin` and `rule` always
//! predict at confidence `1.0` (see `predictors`), so their sweep degenerates
//! to a single step at threshold ≤ 1.0 — not wrong, just uninformative. The
//! sweep is still run generically over every source in `report::run` because
//! it's the same code path a future confidence-bearing source (e.g. #90's
//! encoder) needs, with zero new plumbing once it exists.

use super::corpus::LabeledExample;
use super::predictors::Prediction;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ThresholdPoint {
    pub threshold: f64,
    pub n_total: u64,
    pub n_predicted: u64,
    pub n_correct: u64,
    /// `None` when nothing cleared the threshold — undefined, not 0.0.
    pub precision: Option<f64>,
    pub coverage: f64,
}

/// At each threshold, a prediction only counts if `confidence >= threshold`
/// (otherwise it's treated as an abstention for that threshold's stats).
/// Predicts once per example up front and reuses the results across every
/// threshold rather than recomputing.
pub fn threshold_sweep(
    examples: &[LabeledExample],
    predict: impl Fn(&LabeledExample) -> Prediction,
    thresholds: &[f64],
) -> Vec<ThresholdPoint> {
    let predicted: Vec<(Prediction, &str)> = examples
        .iter()
        .map(|ex| (predict(ex), ex.category.as_str()))
        .collect();
    let n_total = examples.len() as u64;

    thresholds
        .iter()
        .map(|&t| {
            let mut n_predicted = 0u64;
            let mut n_correct = 0u64;
            for (pred, actual) in &predicted {
                if let Some(cat) = &pred.category {
                    if pred.confidence >= t {
                        n_predicted += 1;
                        if cat == actual {
                            n_correct += 1;
                        }
                    }
                }
            }
            ThresholdPoint {
                threshold: t,
                n_total,
                n_predicted,
                n_correct,
                precision: if n_predicted == 0 {
                    None
                } else {
                    Some(n_correct as f64 / n_predicted as f64)
                },
                coverage: if n_total == 0 {
                    0.0
                } else {
                    n_predicted as f64 / n_total as f64
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: &str, category: &str) -> LabeledExample {
        LabeledExample {
            id: id.into(),
            merchant_text: format!("{id} text"),
            merchant_id: id.into(),
            category: category.into(),
            notes: None,
        }
    }

    #[test]
    fn higher_threshold_never_increases_coverage() {
        // Three examples with confidences 0.9 (correct), 0.6 (correct), 0.3 (wrong).
        let examples = vec![
            ex("a", "dining"),
            ex("b", "groceries"),
            ex("c", "transport"),
        ];
        let predict = |e: &LabeledExample| match e.id.as_str() {
            "a" => Prediction::of("dining", 0.9),
            "b" => Prediction::of("groceries", 0.6),
            _ => Prediction::of("shopping", 0.3), // wrong category, low confidence
        };
        let thresholds = [0.0, 0.5, 0.8, 0.95];
        let points = threshold_sweep(&examples, predict, &thresholds);

        // Monotonic non-increasing coverage as threshold rises.
        for w in points.windows(2) {
            assert!(
                w[1].coverage <= w[0].coverage,
                "coverage must not increase as threshold rises: {:?} -> {:?}",
                w[0],
                w[1]
            );
        }
        // At threshold 0.0: all 3 predicted, 2 correct -> precision 2/3.
        assert_eq!(points[0].n_predicted, 3);
        assert_eq!(points[0].n_correct, 2);
        // At threshold 0.95: nothing clears -> undefined precision, 0 coverage.
        let top = points.last().unwrap();
        assert_eq!(top.n_predicted, 0);
        assert_eq!(top.precision, None);
        assert_eq!(top.coverage, 0.0);
    }

    #[test]
    fn raising_threshold_can_increase_precision_by_dropping_a_wrong_low_confidence_call() {
        let examples = vec![ex("a", "dining"), ex("b", "groceries")];
        let predict = |e: &LabeledExample| match e.id.as_str() {
            "a" => Prediction::of("dining", 0.95), // correct, high confidence
            _ => Prediction::of("shopping", 0.4),  // wrong, low confidence
        };
        let points = threshold_sweep(&examples, predict, &[0.0, 0.5]);
        assert_eq!(
            points[0].precision,
            Some(0.5),
            "both counted at threshold 0.0"
        );
        assert_eq!(
            points[1].precision,
            Some(1.0),
            "only the correct high-confidence call survives at 0.5"
        );
        assert_eq!(points[1].coverage, 0.5);
    }
}
