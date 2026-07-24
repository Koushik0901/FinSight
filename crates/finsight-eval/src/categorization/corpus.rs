//! Labeled categorization corpus format (issue #89) + loader.
//!
//! See `eval/CATEGORIZATION_CORPUS.md` for the full format spec, labeling
//! methodology, and what a real (non-synthetic) corpus acquisition effort
//! would need.

use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;

/// One row of the labeled categorization corpus: a transaction description as
/// a categorizer would see it, its normalized merchant identity, and the
/// ground-truth category.
///
/// **SYNTHETIC WARNING:** every corpus file shipped in this repo today
/// (`eval/categorization_corpus.synthetic.jsonl`) is invented data — clearly
/// fictional merchant names chosen to exercise the harness end-to-end, not a
/// real-world precision benchmark. Real ground truth does not exist in this
/// repo (see issue #89).
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

/// Load a corpus from the JSONL format documented in
/// `eval/CATEGORIZATION_CORPUS.md`: one [`LabeledExample`] per line, blank
/// lines and `//`-prefixed comment lines skipped (mirrors
/// `eval/benchmark.jsonl`'s convention, see `crate::main`).
pub fn load_corpus_jsonl(path: impl AsRef<Path>) -> anyhow::Result<Vec<LabeledExample>> {
    let path = path.as_ref();
    let file = std::fs::File::open(path)
        .with_context(|| format!("opening corpus {}", path.display()))?;
    let mut out = Vec::new();
    for (i, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        let example: LabeledExample = serde_json::from_str(line)
            .with_context(|| format!("parsing corpus line {} of {}", i + 1, path.display()))?;
        out.push(example);
    }
    Ok(out)
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
        *examples_per_merchant.entry(ex.merchant_id.clone()).or_insert(0) += 1;
        *category_distribution.entry(ex.category.clone()).or_insert(0) += 1;
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

    #[test]
    fn loads_the_bundled_synthetic_seed_corpus() {
        // Locate the repo-root eval/ dir relative to this crate (CARGO_MANIFEST_DIR
        // is crates/finsight-eval), so this test works regardless of the process's
        // current working directory.
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../eval/categorization_corpus.synthetic.jsonl");
        let examples = load_corpus_jsonl(&path).expect("bundled synthetic corpus must parse");
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
        let dir = std::env::temp_dir().join(format!("finsight-eval-corpus-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mini.jsonl");
        std::fs::write(
            &path,
            "// a header comment\n\n{\"id\":\"1\",\"merchant_text\":\"X\",\"merchant_id\":\"m-x\",\"category\":\"dining\"}\n",
        )
        .unwrap();
        let examples = load_corpus_jsonl(&path).unwrap();
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].category, "dining");
        std::fs::remove_dir_all(&dir).ok();
    }
}
