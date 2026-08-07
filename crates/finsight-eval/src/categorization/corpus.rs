//! Labeled categorization corpus format (issue #89) + loader.
//!
//! See `eval/CATEGORIZATION_CORPUS.md` for the full format spec, labeling
//! methodology, and what a real (non-synthetic) corpus acquisition effort
//! would need.

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::io::BufRead;
use std::path::Path;

/// Where a corpus file's labels came from — declared BY the corpus file
/// itself (`// provenance: synthetic` / `// provenance: real` directive), not
/// asserted by the harness.
///
/// This exists so the report's caveat is *derived from the input* rather than
/// hardcoded. The property being protected is the whole point of this harness:
/// **a synthetic-data-derived number must never be launderable into a real
/// precision claim.** A hardcoded caveat fails that in both directions — a
/// real corpus would silently inherit synthetic language, and (once someone
/// edits the literal to fix that) the synthetic seed would silently lose its
/// warning. Declaring provenance per-file, with a hard error when it is
/// missing, makes both directions impossible without an explicit, reviewable
/// edit to the corpus file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CorpusProvenance {
    /// Invented data. Exercises the harness; proves nothing about real-world
    /// precision.
    Synthetic,
    /// Human-labeled real transactions. Still only as trustworthy as the
    /// labeling methodology, but not fabricated.
    Real,
    /// Generated transaction rows built from a REAL merchant vocabulary and
    /// real bank descriptor grammar — e.g. the MIT-licensed
    /// `DoDataThings/us-bank-transaction-categories-v2`, whose 68k rows compose
    /// 500+ actual merchant names (`PUBLIX`, `WAL-MART #0006`, `PYPL*365
    /// MARKET`) into eight real US statement formats.
    ///
    /// This exists because forcing such a corpus into one of the two values
    /// above would misreport it in whichever direction was chosen:
    ///
    /// - Calling it [`Self::Synthetic`] would attach a caveat reading "invented,
    ///   clearly-fictional transactions", which is false about the part that
    ///   carries the signal. The merchant→category relationship is REAL, and
    ///   that relationship is the entire thing a semantic categorizer learns.
    /// - Calling it [`Self::Real`] would claim human-assigned ground truth over
    ///   actual spending. No human labeled these, and no real ledger produced
    ///   them.
    ///
    /// What it genuinely buys over [`Self::Synthetic`]: real merchant strings
    /// have real semantic content an encoder can succeed or fail on, and real
    /// descriptor noise (store numbers, ACH trace ids, processor prefixes,
    /// inconsistent casing) is exactly the formatting-sensitivity an invented
    /// corpus cannot test. What it still cannot support: a claim about how the
    /// categorizer performs on a specific user's real ledger.
    SemiSynthetic,
}

impl CorpusProvenance {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synthetic => "synthetic",
            Self::Real => "real",
            Self::SemiSynthetic => "semi-synthetic",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "synthetic" => Some(Self::Synthetic),
            "real" => Some(Self::Real),
            "semi-synthetic" | "semi_synthetic" => Some(Self::SemiSynthetic),
            _ => None,
        }
    }

    /// The machine-readable warning that goes into every report computed from
    /// a corpus of this provenance. Derived, never hardcoded at the report
    /// site — see the type doc.
    ///
    /// Invariant pinned by tests: the `Synthetic` text says "synthetic" and
    /// disclaims a measured real-world claim; the `Real` text does NOT contain
    /// the word "synthetic" at all.
    pub fn caveat(self) -> String {
        match self {
            Self::Synthetic => "SYNTHETIC SEED DATA baseline — the corpus file declares \
                 `provenance: synthetic`: invented, clearly-fictional transactions that exist to \
                 validate this harness end to end. This is NOT a measured real-world precision \
                 claim, and no figure below may be quoted as one."
                .to_string(),
            Self::Real => "REAL LABELED DATA — the corpus file declares `provenance: real`, so \
                 these numbers are computed against transactions with human-assigned ground \
                 truth. They are still only as trustworthy as the labeling methodology and the \
                 size of the merchant-disjoint holdout; see eval/CATEGORIZATION_CORPUS.md before \
                 quoting any figure as a validated precision claim."
                .to_string(),
            Self::SemiSynthetic => "SEMI-SYNTHETIC baseline — the corpus file declares \
                 `provenance: semi-synthetic`: generated rows built from a REAL merchant \
                 vocabulary and real bank descriptor formats. The merchant-to-category \
                 signal is real, so these numbers say something an invented corpus cannot \
                 — but no human labeled them and no real ledger produced them, so this is \
                 NOT a measured claim about performance on anyone's actual transactions."
                .to_string(),
        }
    }
}

/// One row of the labeled categorization corpus: a transaction description as
/// a categorizer would see it, its normalized merchant identity, and the
/// ground-truth category.
///
/// **SYNTHETIC WARNING:** every corpus file shipped in this repo today
/// declares `// provenance: synthetic` — invented data, clearly fictional
/// merchant names chosen to exercise the harness end-to-end, not a real-world
/// precision benchmark. Real ground truth does not exist in this repo (see
/// issue #89). That warning travels with the file, not with this comment: see
/// [`CorpusProvenance`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LabeledExample {
    /// Free-form id for traceability. Not used by the split or any matcher.
    pub id: String,
    /// Raw description text as a categorizer would see it (the
    /// `transactions.merchant_raw` equivalent). This is what
    /// `predictors::predict_builtin` / `predict_rule` match against.
    pub merchant_text: String,
    /// Normalized merchant identity — the field the merchant-disjoint split
    /// (`super::split`) partitions on. Multiple `merchant_text` variants of
    /// the same real-world merchant (different store numbers, punctuation,
    /// "renewal" suffixes) should share one `merchant_id` so the split keeps
    /// them together instead of leaking the same merchant across both halves.
    pub merchant_id: String,
    /// Ground-truth category id. Uses the same ids as
    /// `finsight_core::categorize`'s starter categories (groceries, dining,
    /// transport, shopping, travel, gifts, housing, utilities,
    /// subscriptions, health) so `builtin`/`rule` predictions are directly
    /// comparable without a translation layer.
    pub category: String,
    /// Optional free-text note on why this example was included (e.g. a
    /// deliberate "trap" case, or "no keyword overlap by design").
    #[serde(default)]
    pub notes: Option<String>,
}

/// A corpus file's contents plus the provenance it declared. Returned as one
/// unit so a caller cannot obtain the examples without also obtaining the
/// provenance that governs how the resulting numbers may be described.
#[derive(Debug, Clone)]
pub struct LoadedCorpus {
    /// Declared by the file's `// provenance:` directive. Never inferred.
    pub provenance: CorpusProvenance,
    /// The path this corpus was read from, for traceability in logs.
    pub source_path: String,
    pub examples: Vec<LabeledExample>,
}

/// Load a corpus from the JSONL format documented in
/// `eval/CATEGORIZATION_CORPUS.md`: one [`LabeledExample`] per line, blank
/// lines and `//`-prefixed comment lines skipped (mirrors
/// `eval/benchmark.jsonl`'s convention, see `crate::main`).
///
/// Two things this does beyond parsing:
///
/// 1. **Provenance is required.** Exactly one comment line must read
///    `// provenance: synthetic` or `// provenance: real`. A file that
///    declares none is a hard error, so neither a real corpus nor a synthetic
///    one can silently inherit the other's caveat (see [`CorpusProvenance`]).
///    The directive is parsed *before* comment lines are skipped.
/// 2. **`merchant_id` is validated** ([`validate_corpus`]) — blank ids and
///    near-duplicate spellings of the same identity both become parse-time
///    failures instead of silent merchant-disjoint-split violations.
pub fn load_corpus_jsonl(path: impl AsRef<Path>) -> anyhow::Result<LoadedCorpus> {
    let path = path.as_ref();
    let file =
        std::fs::File::open(path).with_context(|| format!("opening corpus {}", path.display()))?;
    let mut provenance: Option<CorpusProvenance> = None;
    let mut out = Vec::new();
    for (i, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line?;
        let line = line.trim();
        // Directive parsing MUST come before the comment skip below — the
        // directive is itself a `//` line.
        if let Some(comment) = line.strip_prefix("//") {
            let lowered = comment.trim().to_ascii_lowercase();
            if let Some(value) = lowered.strip_prefix("provenance:") {
                let parsed = CorpusProvenance::parse(value).ok_or_else(|| {
                    anyhow!(
                        "corpus {} line {}: unknown provenance value {:?} — expected `synthetic` or `real`",
                        path.display(),
                        i + 1,
                        value.trim()
                    )
                })?;
                if let Some(prev) = provenance {
                    bail!(
                        "corpus {} line {}: duplicate `provenance:` directive (already declared `{}`, now `{}`) — a corpus must declare its provenance exactly once",
                        path.display(),
                        i + 1,
                        prev.as_str(),
                        parsed.as_str()
                    );
                }
                provenance = Some(parsed);
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        let example: LabeledExample = serde_json::from_str(line)
            .with_context(|| format!("parsing corpus line {} of {}", i + 1, path.display()))?;
        out.push(example);
    }

    let provenance = provenance.ok_or_else(|| {
        anyhow!(
            "corpus {} declares no provenance — add a `// provenance: synthetic` or \
             `// provenance: real` comment line (see eval/CATEGORIZATION_CORPUS.md). This is \
             required, not optional: the report's caveat is derived from it, so an undeclared \
             corpus could otherwise have a synthetic warning attached to real data or stripped \
             from invented data.",
            path.display()
        )
    })?;

    validate_corpus(&out).with_context(|| format!("validating corpus {}", path.display()))?;

    Ok(LoadedCorpus {
        provenance,
        source_path: path.display().to_string(),
        examples: out,
    })
}

/// Normalized merchant identity used only to DETECT near-duplicate
/// `merchant_id` spellings. The split itself deliberately still partitions on
/// the exact string — normalizing there would paper over the data problem;
/// erroring here forces the corpus to be fixed.
fn normalized_merchant_id(raw: &str) -> String {
    raw.trim().to_lowercase()
}

/// Reject corpora whose `merchant_id` values would make the merchant-disjoint
/// split's guarantee vacuous.
///
/// The split (`super::split`) partitions on **exact string equality** of
/// `merchant_id`. Two consequences, both silent without this check:
///
/// - `"m-brightloaf"` and `"M-Brightloaf "` are one real merchant but two
///   split keys, so they can land on opposite sides of the holdout while
///   `merchant_sets_disjoint` still reports `true` — a real leak that passes
///   the disjointness test.
/// - An empty/whitespace `merchant_id` buckets every such row into one
///   pseudo-merchant, silently coupling unrelated rows.
///
/// Both become parse-time failures instead.
pub fn validate_corpus(examples: &[LabeledExample]) -> anyhow::Result<()> {
    let mut by_normalized: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for ex in examples {
        if ex.merchant_id.trim().is_empty() {
            bail!(
                "example {:?} has a blank merchant_id — the merchant-disjoint split partitions \
                 on this field, so blank ids collapse unrelated rows into one pseudo-merchant",
                ex.id
            );
        }
        by_normalized
            .entry(normalized_merchant_id(&ex.merchant_id))
            .or_default()
            .insert(ex.merchant_id.clone());
    }
    for (normalized, spellings) in &by_normalized {
        if spellings.len() > 1 {
            bail!(
                "merchant_id spellings {:?} all normalize to {:?} — the merchant-disjoint split \
                 compares merchant_id by exact string equality, so these would be treated as \
                 different merchants and could land on opposite sides of the holdout while \
                 actually being one merchant (a leak `merchant_sets_disjoint` would report as \
                 clean). Pick one canonical spelling.",
                spellings.iter().cloned().collect::<Vec<_>>(),
                normalized
            );
        }
    }
    Ok(())
}

/// Volume + diversity stats for a loaded corpus — the numbers issue #89's
/// acceptance criteria asks to be documented ("count of unique merchants,
/// count of labels per merchant, category distribution").
#[derive(Debug, Clone, Serialize)]
pub struct CorpusStats {
    pub total_examples: usize,
    pub unique_merchants: usize,
    /// merchant_id -> number of labeled examples for that merchant.
    pub examples_per_merchant: BTreeMap<String, usize>,
    /// category -> number of labeled examples in that category.
    pub category_distribution: BTreeMap<String, usize>,
}

pub fn corpus_stats(examples: &[LabeledExample]) -> CorpusStats {
    let mut examples_per_merchant: BTreeMap<String, usize> = BTreeMap::new();
    let mut category_distribution: BTreeMap<String, usize> = BTreeMap::new();
    for ex in examples {
        *examples_per_merchant
            .entry(ex.merchant_id.clone())
            .or_insert(0) += 1;
        *category_distribution
            .entry(ex.category.clone())
            .or_insert(0) += 1;
    }
    CorpusStats {
        total_examples: examples.len(),
        unique_merchants: examples_per_merchant.len(),
        examples_per_merchant,
        category_distribution,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(id: &str, merchant_id: &str, text: &str, category: &str) -> LabeledExample {
        LabeledExample {
            id: id.into(),
            merchant_text: text.into(),
            merchant_id: merchant_id.into(),
            category: category.into(),
            notes: None,
        }
    }

    #[test]
    fn stats_counts_merchants_and_categories() {
        let examples = vec![
            ex("1", "m-a", "A Store #1", "groceries"),
            ex("2", "m-a", "A Store #2", "groceries"),
            ex("3", "m-b", "B Cafe", "dining"),
        ];
        let stats = corpus_stats(&examples);
        assert_eq!(stats.total_examples, 3);
        assert_eq!(stats.unique_merchants, 2);
        assert_eq!(stats.examples_per_merchant.get("m-a"), Some(&2));
        assert_eq!(stats.examples_per_merchant.get("m-b"), Some(&1));
        assert_eq!(stats.category_distribution.get("groceries"), Some(&2));
        assert_eq!(stats.category_distribution.get("dining"), Some(&1));
    }

    /// Write a temp corpus file and return its path (plus the dir to clean up).
    fn temp_corpus(name: &str, body: &str) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "finsight-eval-corpus-{}-{}-{name}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{name}.jsonl"));
        std::fs::write(&path, body).unwrap();
        (path, dir)
    }

    #[test]
    fn loads_the_bundled_synthetic_seed_corpus() {
        // Locate the repo-root eval/ dir relative to this crate (CARGO_MANIFEST_DIR
        // is crates/finsight-eval), so this test works regardless of the process's
        // current working directory.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../eval/categorization_corpus.synthetic.jsonl");
        let loaded = load_corpus_jsonl(&path).expect("bundled synthetic corpus must parse");
        let examples = loaded.examples;
        assert!(
            examples.len() >= 20,
            "expected a couple dozen synthetic examples, got {}",
            examples.len()
        );
        let stats = corpus_stats(&examples);
        assert!(
            stats.unique_merchants >= 15,
            "expected broad merchant diversity, got {} unique merchants",
            stats.unique_merchants
        );
        assert!(
            stats.category_distribution.len() >= 8,
            "expected the synthetic seed to span most starter categories, got {}",
            stats.category_distribution.len()
        );
    }

    #[test]
    fn skips_comment_and_blank_lines() {
        let (path, dir) = temp_corpus(
            "mini",
            "// a header comment\n// provenance: synthetic\n\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"m-x\",\"category\":\"dining\"}\n",
        );
        let loaded = load_corpus_jsonl(&path).unwrap();
        assert_eq!(loaded.examples.len(), 1);
        assert_eq!(loaded.examples[0].category, "dining");
        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Provenance (finding #1): the caveat must be derived from the corpus,
    // and neither direction may happen silently. ─────────────────────────────

    /// The bundled seed must keep declaring itself synthetic. Deleting or
    /// flipping that directive is exactly the "synthetic seed silently loses
    /// its warning" failure this test exists to block.
    #[test]
    fn bundled_seed_declares_synthetic_provenance() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../eval/categorization_corpus.synthetic.jsonl");
        let loaded = load_corpus_jsonl(&path).expect("bundled synthetic corpus must parse");
        assert_eq!(loaded.provenance, CorpusProvenance::Synthetic);
    }

    /// The other direction: a corpus declaring `real` loads as real, so a real
    /// corpus cannot inherit synthetic language.
    #[test]
    fn a_corpus_declaring_real_loads_as_real() {
        let (path, dir) = temp_corpus(
            "real",
            "// provenance: real\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"m-x\",\"category\":\"dining\"}\n",
        );
        let loaded = load_corpus_jsonl(&path).unwrap();
        assert_eq!(loaded.provenance, CorpusProvenance::Real);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// No default, in either direction: an undeclared corpus is a hard error
    /// rather than quietly picking up (or losing) the synthetic caveat.
    #[test]
    fn a_corpus_with_no_provenance_directive_is_rejected() {
        let (path, dir) = temp_corpus(
            "undeclared",
            "// just a comment, no directive\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"m-x\",\"category\":\"dining\"}\n",
        );
        let err = load_corpus_jsonl(&path).expect_err("a corpus with no provenance must not load");
        assert!(
            format!("{err:#}").contains("declares no provenance"),
            "error must name the missing directive, got: {err:#}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn an_unknown_provenance_value_is_rejected() {
        let (path, dir) = temp_corpus(
            "bogus",
            "// provenance: mostly-real\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"m-x\",\"category\":\"dining\"}\n",
        );
        let err = load_corpus_jsonl(&path).expect_err("an unknown provenance value must not load");
        assert!(
            format!("{err:#}").contains("unknown provenance value"),
            "got: {err:#}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn duplicate_provenance_directives_are_rejected() {
        let (path, dir) = temp_corpus(
            "dupe",
            "// provenance: synthetic\n// provenance: real\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"m-x\",\"category\":\"dining\"}\n",
        );
        let err = load_corpus_jsonl(&path).expect_err("two conflicting directives must not load");
        assert!(format!("{err:#}").contains("duplicate"), "got: {err:#}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Pin the prose at the enum, so a future edit to the `real` caveat can't
    /// reintroduce synthetic language without failing a test.
    #[test]
    fn caveat_text_matches_provenance() {
        let synthetic = CorpusProvenance::Synthetic.caveat().to_lowercase();
        assert!(synthetic.contains("synthetic"));
        assert!(
            synthetic.contains("not a measured"),
            "synthetic caveat must disclaim a measured real-world claim, got: {synthetic}"
        );
        let real = CorpusProvenance::Real.caveat().to_lowercase();
        assert!(
            !real.contains("synthetic"),
            "a real corpus's caveat must not contain the word 'synthetic' anywhere, got: {real}"
        );
    }

    // ── merchant_id validation (finding #3) ──────────────────────────────────

    #[test]
    fn near_duplicate_merchant_id_spellings_are_rejected() {
        // Byte-different, same real merchant: trailing space + different case.
        // Left unchecked these become two split keys for one merchant, so the
        // split can leak while `merchant_sets_disjoint` still returns true.
        let examples = vec![
            ex("1", "m-brightloaf", "Brightloaf Grocers #12", "groceries"),
            ex("2", "M-Brightloaf ", "Brightloaf Grocers #45", "groceries"),
        ];
        let err =
            validate_corpus(&examples).expect_err("near-duplicate spellings must be rejected");
        let msg = format!("{err:#}");
        assert!(
            msg.contains("m-brightloaf"),
            "error must name the spellings, got: {msg}"
        );
        assert!(
            msg.contains("M-Brightloaf"),
            "error must name the spellings, got: {msg}"
        );
    }

    #[test]
    fn a_repeated_identical_merchant_id_is_fine() {
        // Negative control: the same exact id on multiple rows is the normal,
        // required case (one merchant, several transactions) and must pass.
        let examples = vec![
            ex("1", "m-brightloaf", "Brightloaf Grocers #12", "groceries"),
            ex("2", "m-brightloaf", "Brightloaf Grocers #45", "groceries"),
            ex("3", "m-other", "Other", "dining"),
        ];
        validate_corpus(&examples).expect("identical repeated ids are legitimate");
    }

    #[test]
    fn a_blank_merchant_id_is_rejected() {
        for blank in ["", "   ", "\t"] {
            let examples = vec![ex("row-1", blank, "X", "dining")];
            let err = validate_corpus(&examples)
                .expect_err(&format!("blank merchant_id {blank:?} must be rejected"));
            assert!(
                format!("{err:#}").contains("blank merchant_id"),
                "error must name the problem, got: {err:#}"
            );
        }
    }

    #[test]
    fn a_blank_merchant_id_fails_at_load_time() {
        let (path, dir) = temp_corpus(
            "blank-id",
            "// provenance: synthetic\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"  \",\"category\":\"dining\"}\n",
        );
        let err = load_corpus_jsonl(&path).expect_err("a blank merchant_id must fail the load");
        assert!(
            format!("{err:#}").contains("blank merchant_id"),
            "got: {err:#}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn near_duplicate_merchant_ids_fail_at_load_time() {
        let (path, dir) = temp_corpus(
            "near-dupe",
            "// provenance: synthetic\n\
             {\"id\":\"1\",\"merchant_text\":\"Brightloaf #12\",\"merchant_id\":\"m-brightloaf\",\"category\":\"groceries\"}\n\
             {\"id\":\"2\",\"merchant_text\":\"Brightloaf #45\",\"merchant_id\":\"m-Brightloaf\",\"category\":\"groceries\"}\n",
        );
        let err = load_corpus_jsonl(&path).expect_err("near-duplicate ids must fail the load");
        assert!(format!("{err:#}").contains("normalize to"), "got: {err:#}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
