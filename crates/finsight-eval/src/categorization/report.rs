//! Orchestrates the merchant-disjoint split, confusion matrices, and
//! threshold sweeps into one JSON-serializable report — what
//! `src/bin/categorization_eval.rs` runs and prints.

use super::confusion::ConfusionMatrix;
use super::corpus::{corpus_stats, CorpusStats, LabeledExample};
use super::predictors::{predict_builtin_for, predict_rule, synthetic_rules, Prediction};
use super::split::merchant_disjoint_split;
use super::threshold::{threshold_sweep, ThresholdPoint};
use serde::Serialize;

/// Confidence cutoffs swept for every source. `builtin`/`rule` always predict
/// at 1.0 (see `predictors`), so their curve is a single step; a future
/// confidence-bearing source uses this same list with no code changes.
pub const DEFAULT_THRESHOLDS: &[f64] = &[0.0, 0.3, 0.5, 0.6, 0.7, 0.8, 0.9, 0.95, 1.0];

#[derive(Debug, Clone, Serialize)]
pub struct ConfusionSummary {
    pub n_total: u64,
    pub n_predicted: u64,
    pub n_correct: u64,
    pub precision: Option<f64>,
    pub coverage: f64,
    /// Full predicted×actual counts, for anyone who wants the raw breakdown.
    pub matrix: ConfusionMatrix,
}

fn summarize(matrix: ConfusionMatrix) -> ConfusionSummary {
    ConfusionSummary {
        n_total: matrix.total,
        n_predicted: matrix.n_predicted(),
        n_correct: matrix.n_correct(),
        precision: matrix.precision(),
        coverage: matrix.coverage(),
        matrix,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceReport {
    pub source: String,
    /// Confusion + precision/coverage over the WHOLE corpus (reference +
    /// holdout together) — an in-sample number, included for visibility but
    /// not the generalization claim.
    pub full_corpus: ConfusionSummary,
    /// Confusion + precision/coverage over ONLY the merchant-disjoint holdout
    /// half — the number that actually speaks to "does this generalize to
    /// merchants not seen in the reference half," which is what the epic's
    /// ≥98%-merchant-disjoint gate is about.
    pub holdout_only: ConfusionSummary,
    /// Threshold sweep computed over the full corpus.
    pub threshold_sweep: Vec<ThresholdPoint>,
}

fn source_report(
    name: &str,
    full: &[LabeledExample],
    holdout: &[LabeledExample],
    thresholds: &[f64],
    predict: impl Fn(&LabeledExample) -> Prediction,
) -> SourceReport {
    let full_matrix = ConfusionMatrix::build(name, full, &predict);
    let holdout_matrix = ConfusionMatrix::build(name, holdout, &predict);
    let sweep = threshold_sweep(full, &predict, thresholds);
    SourceReport {
        source: name.to_string(),
        full_corpus: summarize(full_matrix),
        holdout_only: summarize(holdout_matrix),
        threshold_sweep: sweep,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorizationEvalReport {
    /// Machine-readable warning baked into the report itself — not just prose
    /// documentation — so nothing downstream (a dashboard, a CI gate, a human
    /// skimming JSON) can mistake this for a validated real-world number.
    pub caveat: String,
    pub corpus_stats: CorpusStats,
    pub holdout_fraction: f64,
    pub split_seed: u64,
    pub sources: Vec<SourceReport>,
}

/// Run the full harness: merchant-disjoint split, then confusion matrix +
/// threshold sweep for the `builtin` and `rule` sources (the only two this
/// session can honestly baseline — see `super` module docs for why `llm` is
/// not included).
pub fn run(examples: &[LabeledExample], holdout_fraction: f64, seed: u64) -> CategorizationEvalReport {
    let (_reference, holdout) = merchant_disjoint_split(examples, holdout_fraction, seed);
    let rules = synthetic_rules();

    let sources = vec![
        source_report("builtin", examples, &holdout, DEFAULT_THRESHOLDS, predict_builtin_for),
        source_report(
            "rule",
            examples,
            &holdout,
            DEFAULT_THRESHOLDS,
            move |ex: &LabeledExample| predict_rule(&ex.merchant_text, &rules),
        ),
    ];

    CategorizationEvalReport {
        caveat: "SYNTHETIC SEED DATA baseline — computed against invented, clearly-fictional \
                 transactions (eval/categorization_corpus.synthetic.jsonl) for harness \
                 end-to-end validation. This is NOT a measured real-world precision claim. \
                 A real corpus (issue #89) does not exist in this repo yet."
            .to_string(),
        corpus_stats: corpus_stats(examples),
        holdout_fraction,
        split_seed: seed,
        sources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categorization::corpus::load_corpus_jsonl;
    use std::path::Path;

    fn bundled_corpus() -> Vec<LabeledExample> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../eval/categorization_corpus.synthetic.jsonl");
        load_corpus_jsonl(&path).expect("bundled synthetic corpus must load")
    }

    #[test]
    fn report_caveat_is_present_and_says_synthetic() {
        let examples = bundled_corpus();
        let report = run(&examples, 0.3, 42);
        let lower = report.caveat.to_lowercase();
        assert!(lower.contains("synthetic"));
        assert!(
            lower.contains("not a measured"),
            "must explicitly disclaim this as a measured real-world claim, got: {}",
            report.caveat
        );
    }

    #[test]
    fn report_has_builtin_and_rule_sources_with_sane_numbers() {
        let examples = bundled_corpus();
        let report = run(&examples, 0.3, 42);
        assert_eq!(report.sources.len(), 2);

        let builtin = report.sources.iter().find(|s| s.source == "builtin").unwrap();
        let rule = report.sources.iter().find(|s| s.source == "rule").unwrap();

        // Non-degenerate: the corpus was constructed so builtin actually
        // fires on a meaningful chunk of examples (public-brand + generic
        // keyword rows) and, because the ground truth was authored to agree
        // with the real KEYWORD_MAP except for one deliberate trap row,
        // precision should be high but not artificially 100%.
        assert!(builtin.full_corpus.coverage > 0.0, "builtin must cover more than nothing");
        assert!(builtin.full_corpus.coverage < 1.0, "builtin must not cover everything (honest gaps exist)");
        let builtin_precision = builtin.full_corpus.precision.expect("builtin made at least one prediction");
        assert!(builtin_precision > 0.5, "builtin precision should be reasonably high on this corpus, got {builtin_precision}");
        assert!(builtin_precision < 1.0, "the deliberate trap row (Esso Corner Store) must cost at least one point of precision");

        assert!(rule.full_corpus.coverage > 0.0, "the synthetic rules must match at least one row");
        assert_eq!(rule.full_corpus.precision, Some(1.0), "synthetic rules were authored to be correct wherever they fire");

        // Sanity: every count is internally consistent (predicted <= total, correct <= predicted).
        for s in &report.sources {
            assert!(s.full_corpus.n_predicted <= s.full_corpus.n_total);
            assert!(s.full_corpus.n_correct <= s.full_corpus.n_predicted);
            assert!(s.holdout_only.n_predicted <= s.holdout_only.n_total);
            assert!(s.holdout_only.n_correct <= s.holdout_only.n_predicted);
        }
    }

    #[test]
    fn report_serializes_to_json() {
        let examples = bundled_corpus();
        let report = run(&examples, 0.3, 42);
        let json = serde_json::to_string(&report).expect("report must serialize");
        assert!(json.contains("\"caveat\""));
        assert!(json.contains("\"builtin\""));
        assert!(json.contains("\"rule\""));
    }
}
