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

/// # Slice 5 calibration (issue #93)
///
/// Pick the **highest-coverage** threshold that still meets the precision gate
/// on the *merchant-disjoint holdout* curve — the only curve #93 allows an
/// auto-apply decision to be made from.
///
/// Returns `None` when NO holdout point clears `min_precision` with at least
/// `min_n_predicted` predictions. That is the honest answer for today's
/// Canadian real-merchant data (see `eval/CENTROID_BASELINE.md`'s sweep: only
/// 100% at n=7 qualifies on precision alone, which fails the `min_n` guard).
/// A `None` means "no confidence band qualifies for auto-apply — proposals
/// stay review-only", not "pick a lower precision".
///
/// `min_n_predicted` is the same statistical floor `private_eval` uses for its
/// "too small to trust" caveat (`MIN_HELDOUT_MERCHANTS_FOR_CONFIDENT_CLAIM`
/// ≈ 30 merchants, approximated here as 30 predictions — one prediction per
/// held-out merchant at the low-n edge where the guard matters). Using a bare
/// precision without an N guard would let a 7-row 100% (the observed tail at
/// 0.75) look like a gate pass.
pub fn calibrated_auto_apply_threshold(
    holdout_sweep: &[ThresholdPoint],
    min_precision: f64,
    min_n_predicted: u64,
) -> Option<ThresholdPoint> {
    // Lowest threshold that still passes is highest coverage, so scan in
    // threshold order and keep the *first* passing point when sweeping low→high.
    // Our DEFAULT_THRESHOLDS are ascending, but callers may supply any order;
    // sort by threshold to make the choice deterministic regardless.
    let mut sorted = holdout_sweep.to_vec();
    sorted.sort_by(|a, b| a.threshold.partial_cmp(&b.threshold).unwrap_or(std::cmp::Ordering::Equal));
    let mut candidate: Option<ThresholdPoint> = None;
    for pt in sorted {
        if let Some(p) = pt.precision {
            if p >= min_precision && pt.n_predicted >= min_n_predicted {
                // First (lowest threshold) that qualifies is highest coverage
                candidate = Some(pt);
                break;
            }
        }
    }
    candidate
}

/// Convenience for the epic #74 gate: ≥98% precision, ≥30 predictions on the
/// merchant-disjoint holdout. Returns `None` when no band qualifies.
pub fn calibrated_threshold_for_gate(holdout_sweep: &[ThresholdPoint]) -> Option<ThresholdPoint> {
    calibrated_auto_apply_threshold(holdout_sweep, 0.98, 30)
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

    #[test]
    fn calibrated_threshold_picks_highest_coverage_band_meeting_gate() {
        // Synthetic sweep: precision rises with threshold but coverage falls.
        let sweep = vec![
            ThresholdPoint { threshold: 0.35, n_total: 100, n_predicted: 80, n_correct: 72, precision: Some(0.90), coverage: 0.80 },
            ThresholdPoint { threshold: 0.60, n_total: 100, n_predicted: 40, n_correct: 39, precision: Some(0.975), coverage: 0.40 },
            ThresholdPoint { threshold: 0.70, n_total: 100, n_predicted: 35, n_correct: 35, precision: Some(1.0), coverage: 0.35 },
        ];
        // At gate 0.98/30: 0.60 fails (0.975), 0.70 passes — should pick 0.70.
        let pt = calibrated_auto_apply_threshold(&sweep, 0.98, 30).unwrap();
        assert!((pt.threshold - 0.70).abs() < 1e-9);
        assert_eq!(pt.n_predicted, 35);
    }

    #[test]
    fn calibrated_threshold_returns_none_when_only_small_n_tail_passes() {
        // Mirrors real CENTROID_BASELINE tail: 100% at n=7, nothing at feasible n.
        let sweep = vec![
            ThresholdPoint { threshold: 0.35, n_total: 574, n_predicted: 522, n_correct: 269, precision: Some(0.515), coverage: 0.909 },
            ThresholdPoint { threshold: 0.70, n_total: 574, n_predicted: 25, n_correct: 22, precision: Some(0.88), coverage: 0.044 },
            ThresholdPoint { threshold: 0.75, n_total: 574, n_predicted: 7, n_correct: 7, precision: Some(1.0), coverage: 0.012 },
        ];
        // Gate 0.98/30: only 0.75 passes precision but fails min_n, so None.
        assert!(calibrated_auto_apply_threshold(&sweep, 0.98, 30).is_none());
        // Even without min_n, 0.75 would be the answer — proves guard matters.
        assert!(calibrated_auto_apply_threshold(&sweep, 0.98, 0).is_some());
    }

    #[test]
    fn calibrated_threshold_is_deterministic_regardless_of_input_order() {
        let mut sweep = vec![
            ThresholdPoint { threshold: 0.70, n_total: 100, n_predicted: 35, n_correct: 35, precision: Some(1.0), coverage: 0.35 },
            ThresholdPoint { threshold: 0.35, n_total: 100, n_predicted: 90, n_correct: 88, precision: Some(0.978), coverage: 0.90 },
        ];
        // 0.35 already fails threshold slightly? adjust to pass both
        sweep[1].precision = Some(0.99);
        let a = calibrated_auto_apply_threshold(&sweep, 0.98, 30).unwrap();
        sweep.reverse();
        let b = calibrated_auto_apply_threshold(&sweep, 0.98, 30).unwrap();
        assert_eq!(a.threshold, b.threshold);
        assert!((a.threshold - 0.35).abs() < 1e-9, "lowest passing threshold should win regardless of order");
    }

    #[test]
    fn calibrated_threshold_uses_holdout_only_convention() {
        // This test documents the contract: caller must pass holdout_sweep,
        // not full_corpus. We can't enforce at type level, but we note it.
        // A point with high in-sample precision that doesn't exist holdout
        // must not be supplied — this passes holdout and proves it still works.
        let holdout = vec![
            ThresholdPoint { threshold: 0.5, n_total: 50, n_predicted: 40, n_correct: 39, precision: Some(0.975), coverage: 0.80 },
        ];
        assert!(calibrated_auto_apply_threshold(&holdout, 0.98, 30).is_none());
    }
}
