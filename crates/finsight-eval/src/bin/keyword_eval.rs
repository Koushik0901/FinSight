//! Precision/coverage of the SHIPPED keyword pass alone, over any corpus.
//!
//! ```text
//! cargo run -p finsight-eval --bin keyword_eval -- eval/categorization_corpus.semi_synthetic.jsonl
//! ```
//!
//! # Why this exists separately from `centroid_eval`
//!
//! `builtin_category` is a lowercase substring scan. It needs no model, no
//! embeddings and no network — it answers in milliseconds over tens of
//! thousands of rows. Reading its number out of `centroid_eval` meant loading
//! MiniLM and embedding the whole corpus twice first, which turned a
//! sub-second question into a ten-minute one for no reason at all.
//!
//! Keep them separate: anyone tuning the keyword table should get an answer
//! fast enough to iterate, and tuning the keyword table is now the highest-value
//! lever there is (see `eval/CENTROID_BASELINE.md` — on Canadian merchants the
//! keyword pass scores 100% precision where the semantic pass scores 51.5%).
//!
//! Reports on the merchant-disjoint holdout, same split parameters as
//! `centroid_eval`, so the two are directly comparable — and additionally on
//! the FULL corpus, because a keyword table has no training half to hold out
//! from. It is hand-written, not fitted, so the holdout/full distinction that
//! matters for a learned model is informational here rather than load-bearing.

use anyhow::{Context, Result};
use finsight_eval::categorization::{
    confusion::ConfusionMatrix, corpus::load_corpus_jsonl, predictors::predict_builtin_for,
    split::merchant_disjoint_split,
};

const HOLDOUT_FRACTION: f64 = 0.3;
const SPLIT_SEED: u64 = 42;

fn main() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .context("usage: keyword_eval <corpus.jsonl>")?;
    let loaded = load_corpus_jsonl(&path).with_context(|| format!("loading {path}"))?;
    let (_reference, holdout) =
        merchant_disjoint_split(&loaded.examples, HOLDOUT_FRACTION, SPLIT_SEED);

    println!("corpus:     {path}");
    println!("provenance: {}", loaded.provenance.as_str());

    println!(
        "\n{:<16} {:>8} {:>10} {:>11} {:>12}",
        "scope", "rows", "coverage", "precision", "correct"
    );
    for (label, rows) in [
        ("full corpus", &loaded.examples),
        ("holdout only", &holdout),
    ] {
        let m = ConfusionMatrix::build("builtin", rows, predict_builtin_for);
        println!(
            "{label:<16} {:>8} {:>7.1}% {:>11} {:>12}",
            rows.len(),
            m.coverage() * 100.0,
            m.precision()
                .map(|p| format!("{:.1}%", p * 100.0))
                .unwrap_or_else(|| "n/a".into()),
            format!("{}/{}", m.n_correct(), m.n_predicted()),
        );
    }

    // Where the keyword table is WRONG, which is the only thing that can make
    // it worse than doing nothing. A miss costs coverage and the user can fix
    // it; a false match writes a wrong category and looks confident.
    let m = ConfusionMatrix::build("builtin", &loaded.examples, predict_builtin_for);
    let mut wrong: Vec<(String, String, String)> = Vec::new();
    for ex in &loaded.examples {
        if let Some(got) = predict_builtin_for(ex).category {
            if got != ex.category {
                wrong.push((ex.category.clone(), got, ex.merchant_text.clone()));
            }
        }
    }
    println!(
        "\nfalse matches: {} of {} predictions",
        wrong.len(),
        m.n_predicted()
    );
    let mut by_pair: std::collections::BTreeMap<(String, String), Vec<String>> =
        std::collections::BTreeMap::new();
    for (actual, got, text) in wrong {
        by_pair.entry((actual, got)).or_default().push(text);
    }
    let mut pairs: Vec<_> = by_pair.into_iter().collect();
    pairs.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    for ((actual, got), texts) in pairs.iter().take(12) {
        println!(
            "  {actual} -> {got} ({})  e.g. {}",
            texts.len(),
            texts
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>()
                .join(" | ")
        );
    }

    println!("\n{}", loaded.provenance.caveat());
    Ok(())
}
