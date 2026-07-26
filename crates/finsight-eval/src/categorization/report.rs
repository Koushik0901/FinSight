//! Orchestrates the merchant-disjoint split, confusion matrices, and
//! threshold sweeps into one JSON-serializable report — what
//! `src/bin/categorization_eval.rs` runs and prints.

use super::confusion::ConfusionMatrix;
use super::corpus::{corpus_stats, CorpusProvenance, CorpusStats, LabeledExample};
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
    /// Threshold sweep over the WHOLE corpus — in-sample, like
    /// [`Self::full_corpus`]. The field name carries the scope because a doc
    /// comment does not serialize: a JSON consumer picking an auto-apply
    /// cutoff off an unscoped `threshold_sweep` would be reading an in-sample
    /// curve that overstates precision relative to the merchant-disjoint
    /// holdout.
    pub threshold_sweep_full_corpus: Vec<ThresholdPoint>,
    /// Threshold sweep over ONLY the merchant-disjoint holdout — the curve an
    /// auto-apply cutoff should actually be chosen from, since it is the one
    /// that speaks to unseen merchants.
    pub threshold_sweep_holdout: Vec<ThresholdPoint>,
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
    SourceReport {
        source: name.to_string(),
        full_corpus: summarize(full_matrix),
        holdout_only: summarize(holdout_matrix),
        threshold_sweep_full_corpus: threshold_sweep(full, &predict, thresholds),
        threshold_sweep_holdout: threshold_sweep(holdout, &predict, thresholds),
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CategorizationEvalReport {
    /// Machine-readable provenance, copied from what the corpus file itself
    /// declared (`// provenance:` directive). Present as its own field so a
    /// consumer can branch on it without string-matching the caveat prose.
    pub corpus_provenance: CorpusProvenance,
    /// Human-readable warning baked into the report itself — not just prose
    /// documentation — so nothing downstream (a dashboard, a CI gate, a human
    /// skimming JSON) can mistake this for a validated real-world number.
    ///
    /// **Derived from [`Self::corpus_provenance`], never hardcoded here.** A
    /// literal string at this site would attach synthetic language to a real
    /// corpus (and, once edited to stop doing that, would strip the warning
    /// from the synthetic seed) — the exact laundering path this harness
    /// exists to prevent.
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
///
/// `provenance` comes from the loaded corpus
/// ([`corpus::LoadedCorpus::provenance`](super::corpus::LoadedCorpus)) and is
/// the sole input to the report's caveat — pass what the file declared, never
/// a guess.
pub fn run(
    examples: &[LabeledExample],
    provenance: CorpusProvenance,
    holdout_fraction: f64,
    seed: u64,
) -> CategorizationEvalReport {
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
        corpus_provenance: provenance,
        caveat: provenance.caveat(),
        corpus_stats: corpus_stats(examples),
        holdout_fraction,
        split_seed: seed,
        sources,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categorization::corpus::{load_corpus_jsonl, LoadedCorpus};
    use std::path::Path;

    /// Split parameters the committed baseline artifact was generated with —
    /// the binary's defaults.
    const BASELINE_HOLDOUT_FRACTION: f64 = 0.3;
    const BASELINE_SEED: u64 = 42;

    fn repo_file(rel: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)
    }

    fn bundled() -> LoadedCorpus {
        load_corpus_jsonl(repo_file("eval/categorization_corpus.synthetic.jsonl"))
            .expect("bundled synthetic corpus must load")
    }

    fn run_bundled() -> CategorizationEvalReport {
        let loaded = bundled();
        run(
            &loaded.examples,
            loaded.provenance,
            BASELINE_HOLDOUT_FRACTION,
            BASELINE_SEED,
        )
    }

    /// Direction 1 of the provenance invariant: the bundled synthetic seed
    /// always produces a report that says synthetic. It gets that from the
    /// file's own directive — nothing in `run()` asserts it.
    #[test]
    fn report_caveat_is_present_and_says_synthetic() {
        let report = run_bundled();
        assert_eq!(report.corpus_provenance, CorpusProvenance::Synthetic);
        let lower = report.caveat.to_lowercase();
        assert!(lower.contains("synthetic"));
        assert!(
            lower.contains("not a measured"),
            "must explicitly disclaim this as a measured real-world claim, got: {}",
            report.caveat
        );
    }

    /// Direction 2, the one a hardcoded literal got wrong: a corpus that does
    /// NOT declare synthetic produces a report whose caveat does not claim
    /// synthetic. Loads the provenance from an actual file so this exercises
    /// the whole load→run path, not just the enum.
    #[test]
    fn a_real_corpus_report_does_not_claim_synthetic() {
        let dir = std::env::temp_dir().join(format!(
            "finsight-eval-report-real-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("real.jsonl");
        std::fs::write(
            &path,
            "// provenance: real\n\
             {\"id\":\"1\",\"merchant_text\":\"Costco Wholesale #331\",\"merchant_id\":\"m-costco\",\"category\":\"groceries\"}\n\
             {\"id\":\"2\",\"merchant_text\":\"Netflix.com\",\"merchant_id\":\"m-netflix\",\"category\":\"subscriptions\"}\n",
        )
        .unwrap();
        let loaded = load_corpus_jsonl(&path).expect("real-provenance corpus must load");
        assert_eq!(loaded.provenance, CorpusProvenance::Real);

        let report = run(&loaded.examples, loaded.provenance, 0.5, 42);
        assert_eq!(report.corpus_provenance, CorpusProvenance::Real);
        assert!(
            !report.caveat.to_lowercase().contains("synthetic"),
            "a real corpus must not inherit the synthetic caveat, got: {}",
            report.caveat
        );
        // And the JSON a downstream consumer reads carries the same verdict.
        let json = serde_json::to_string(&report).unwrap();
        assert!(json.contains("\"corpus_provenance\":\"real\""), "got: {json}");
        assert!(!json.to_lowercase().contains("synthetic"));

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn report_has_builtin_and_rule_sources_with_sane_numbers() {
        let report = run_bundled();
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
        let report = run_bundled();
        let json = serde_json::to_string(&report).expect("report must serialize");
        assert!(json.contains("\"caveat\""));
        assert!(json.contains("\"builtin\""));
        assert!(json.contains("\"rule\""));
    }

    /// Both sweeps carry their scope in the FIELD NAME, not just a doc
    /// comment — a JSON consumer picking an auto-apply cutoff can't tell an
    /// in-sample curve from the merchant-disjoint one otherwise.
    #[test]
    fn threshold_sweeps_are_scoped_in_the_serialized_field_names() {
        let report = run_bundled();
        let json = serde_json::to_string(&report).expect("report must serialize");
        assert!(json.contains("\"threshold_sweep_full_corpus\""));
        assert!(json.contains("\"threshold_sweep_holdout\""));
        assert!(
            !json.contains("\"threshold_sweep\""),
            "the unscoped field name must be gone entirely"
        );
        for s in &report.sources {
            assert_eq!(
                s.threshold_sweep_full_corpus.len(),
                DEFAULT_THRESHOLDS.len()
            );
            assert_eq!(s.threshold_sweep_holdout.len(), DEFAULT_THRESHOLDS.len());
            // Each sweep is scoped to its own population.
            assert_eq!(s.threshold_sweep_full_corpus[0].n_total, s.full_corpus.n_total);
            assert_eq!(s.threshold_sweep_holdout[0].n_total, s.holdout_only.n_total);
        }
    }

    /// The committed baseline artifact is regenerated by hand, so nothing but
    /// a test keeps it in step with the corpus it claims to describe. Editing
    /// `eval/categorization_corpus.synthetic.jsonl` (or any harness logic)
    /// without regenerating the artifact must fail here rather than leaving
    /// two checked-in files that silently disagree.
    #[test]
    fn committed_baseline_artifact_matches_the_bundled_corpus() {
        let path = repo_file("eval/categorization_baseline.synthetic.json");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
        // Compare the SERIALIZED text, not two parsed `Value`s: serde_json's
        // float parser is not always exact to 1 ULP on 17-significant-digit
        // literals like `0.36363636363636365`, so a parse round-trip can
        // report a spurious difference. The artifact is produced by
        // `to_string_pretty` on this very type, so text equality is both
        // exact and the stronger pin. Line endings are normalized because git
        // may check the file out with CRLF on Windows.
        let normalize = |s: &str| s.replace("\r\n", "\n").trim_end().to_string();
        let committed = normalize(&raw);
        let computed = normalize(
            &serde_json::to_string_pretty(&run_bundled()).expect("report must serialize"),
        );

        if computed == committed {
            return;
        }

        // Field-by-field: dumping two ~350-line JSON blobs into a panic is
        // unreadable, so report the first line that actually differs.
        let stale = format!(
            "\n\neval/categorization_baseline.synthetic.json is STALE — it no longer matches a \
             fresh run over eval/categorization_corpus.synthetic.jsonl (holdout_fraction={}, \
             seed={}). Regenerate it:\n\n    cargo run -p finsight-eval --bin \
             categorization_eval -- --out eval/categorization_baseline.synthetic.json\n\n\
             and update the numbers table in eval/CATEGORIZATION_CORPUS.md if they moved.\n",
            BASELINE_HOLDOUT_FRACTION, BASELINE_SEED
        );
        for (i, (a, b)) in computed.lines().zip(committed.lines()).enumerate() {
            if a != b {
                panic!(
                    "{stale}\nfirst difference at line {}:\n  computed:  {}\n  committed: {}\n",
                    i + 1,
                    a.trim(),
                    b.trim()
                );
            }
        }
        panic!(
            "{stale}\nthe two differ in length: computed has {} lines, committed has {}\n",
            computed.lines().count(),
            committed.lines().count()
        );
    }
}
