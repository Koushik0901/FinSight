//! Measures the centroid (prototype) categorizer against the #88 harness —
//! the measurement acceptance criterion of issue #92.
//!
//! ```text
//! cargo run -p finsight-eval --bin centroid_eval -- [corpus.jsonl]
//! ```
//!
//! # This is a separate binary on purpose
//!
//! It loads the REAL sentence encoder, which downloads ~90MB of model weights
//! from HuggingFace on first run and then holds 100-200MB RSS. Nothing in the
//! ordinary test suite may depend on that — it would be slow, network-bound,
//! and would fail in exactly the sandboxed environments where this repo's
//! `samples/`-dependent tests already fail. The scoring logic itself lives in
//! `categorization::centroid_predictor` and IS unit-tested offline.
//!
//! # What the number means, and what it does not
//!
//! Prototypes are built from the **reference** half of a merchant-disjoint
//! split and scored against the **holdout** half, so the headline figure is a
//! generalization claim about merchants the prototypes never saw. It is still
//! only as real as the corpus: the bundled corpora declare
//! `provenance: synthetic`, and a synthetic-derived number may never be
//! restated as a real-world precision claim. The harness carries that caveat
//! from the corpus file into the report rather than letting the caller assert
//! it, and this binary prints it prominently for the same reason.
//!
//! In particular this does NOT discharge epic #74's ≥98% merchant-disjoint
//! auto-apply gate. That gate needs issue #89's real labeled corpus, which is
//! blocked on a real-label source (#94's review surface). Until then the
//! centroid pass stays proposal-only regardless of what this prints.

use anyhow::{Context, Result};
use finsight_eval::categorization::{
    centroid_predictor::{build_prototypes, predict_centroid, Prototype},
    confusion::ConfusionMatrix,
    corpus::{corpus_stats, load_corpus_jsonl},
    predictors::{predict_builtin_for, Prediction},
    split::{merchant_disjoint_split, merchant_sets_disjoint},
};

/// Matches the production pass's floor (`finsight_agent::embedding::centroid::
/// MIN_PROPOSAL_SCORE`) so the measured behaviour is the shipped behaviour.
const MIN_SCORE: f32 = 0.35;
const HOLDOUT_FRACTION: f64 = 0.3;
const SPLIT_SEED: u64 = 42;

#[tokio::main]
async fn main() -> Result<()> {
    let corpus_path = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../eval/categorization_corpus.synthetic_multi_archetype.jsonl")
            .to_string()
    });

    let loaded = load_corpus_jsonl(&corpus_path)
        .with_context(|| format!("loading corpus {corpus_path}"))?;
    let stats = corpus_stats(&loaded.examples);

    let (reference, holdout) =
        merchant_disjoint_split(&loaded.examples, HOLDOUT_FRACTION, SPLIT_SEED);
    // The split's own guarantee, re-asserted here rather than assumed: if a
    // merchant leaked across the halves, every number below would be inflated
    // and would still look perfectly plausible.
    anyhow::ensure!(
        merchant_sets_disjoint(&reference, &holdout),
        "split leaked a merchant across halves — the holdout number would be meaningless"
    );

    println!("corpus:      {corpus_path}");
    println!("provenance:  {}", loaded.provenance.as_str());
    println!(
        "examples:    {} total, {} unique merchants, {} categories",
        stats.total_examples,
        stats.unique_merchants,
        stats.category_distribution.len()
    );
    println!("split:       {} reference / {} holdout (seed {SPLIT_SEED})", reference.len(), holdout.len());

    // --- embed -------------------------------------------------------------
    let data_dir = std::env::var("FINSIGHT_DATA_DIR").unwrap_or_else(|_| "./data".into());
    eprintln!("loading encoder (first run downloads model weights)…");
    let encoder = finsight_agent::embedding::get_encoder(std::path::Path::new(&data_dir))
        .await
        .context("loading the sentence encoder")?;
    println!("encoder:     {} ({} dims)", encoder.model_id(), encoder.dims());

    let reference_texts: Vec<String> =
        reference.iter().map(|e| e.merchant_text.clone()).collect();
    let holdout_texts: Vec<String> = holdout.iter().map(|e| e.merchant_text.clone()).collect();

    let reference_vectors = encoder.embed(&reference_texts).await?;
    let holdout_vectors = encoder.embed(&holdout_texts).await?;

    let prototypes: Vec<Prototype> = build_prototypes(&reference, &reference_vectors);
    println!("prototypes:  {} categories with a usable centroid", prototypes.len());

    // --- score -------------------------------------------------------------
    // Index by example id so the predictor closure can find each holdout row's
    // precomputed vector; `ConfusionMatrix::build` is sync and takes the
    // example, not an embedding.
    let vector_by_id: std::collections::HashMap<&str, &Vec<f32>> = holdout
        .iter()
        .map(|e| e.id.as_str())
        .zip(holdout_vectors.iter())
        .collect();

    let centroid_matrix = ConfusionMatrix::build("centroid", &holdout, |ex| {
        match vector_by_id.get(ex.id.as_str()) {
            Some(v) => predict_centroid(v, &prototypes, MIN_SCORE),
            None => Prediction::abstain(),
        }
    });
    // The deterministic pass on the SAME holdout rows — the comparison issue
    // #92 actually asks for. Comparing against a different population would
    // make the two columns incomparable.
    let builtin_matrix = ConfusionMatrix::build("builtin", &holdout, predict_builtin_for);

    println!("\nmerchant-disjoint holdout ({} rows)", holdout.len());
    println!("{:<10} {:>10} {:>10} {:>10}", "source", "coverage", "precision", "correct");
    for m in [&builtin_matrix, &centroid_matrix] {
        println!(
            "{:<10} {:>9.1}% {:>10} {:>10}",
            m.source,
            m.coverage() * 100.0,
            m.precision()
                .map(|p| format!("{:.1}%", p * 100.0))
                .unwrap_or_else(|| "n/a".into()),
            format!("{}/{}", m.n_correct(), m.n_predicted()),
        );
    }

    // --- error analysis ----------------------------------------------------
    // Aggregate precision says how often the pass is wrong; it says nothing
    // about WHERE. Slice 7 (#95) is a menu of experiments — reranker,
    // multilingual, SetFit, constrained LLM fallback — and picking one without
    // knowing the shape of the failures would be choosing by taste. This is the
    // evidence for that choice.
    let mut confusions: std::collections::BTreeMap<(String, String), Vec<&str>> =
        std::collections::BTreeMap::new();
    let mut abstains: Vec<(&str, String, f32)> = Vec::new();

    for ex in &holdout {
        let Some(v) = vector_by_id.get(ex.id.as_str()) else { continue };
        let pred = predict_centroid(v, &prototypes, MIN_SCORE);
        match pred.category {
            Some(got) if got != ex.category => {
                confusions
                    .entry((ex.category.clone(), got))
                    .or_default()
                    .push(ex.merchant_text.as_str());
            }
            None => {
                // What the top match WOULD have been, had the floor allowed it —
                // an abstain just short of the floor is a different problem from
                // one with no signal at all.
                let unfloored = predict_centroid(v, &prototypes, -1.0);
                abstains.push((
                    ex.merchant_text.as_str(),
                    unfloored.category.unwrap_or_else(|| "<none>".into()),
                    unfloored.confidence as f32,
                ));
            }
            _ => {}
        }
    }

    let n_wrong: usize = confusions.values().map(Vec::len).sum();
    println!("\nerror analysis — {n_wrong} misclassified, {} abstained", abstains.len());
    if !confusions.is_empty() {
        println!("\n  actual -> predicted (count)  examples");
        for ((actual, predicted), texts) in
            // Biggest confusion pairs first: that is where a reranker or an
            // extra example would buy the most.
            {
                let mut v: Vec<_> = confusions.iter().collect();
                v.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(b.0)));
                v
            }
        {
            println!(
                "  {actual} -> {predicted} ({})  e.g. {}",
                texts.len(),
                texts.iter().take(3).cloned().collect::<Vec<_>>().join(" | ")
            );
        }
    }
    if !abstains.is_empty() {
        println!("\n  abstained (top match below the {MIN_SCORE} floor)");
        let mut sorted = abstains.clone();
        sorted.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        for (text, would_be, score) in sorted.iter().take(10) {
            println!("  {score:.3}  {text}  (would have said: {would_be})");
        }
    }

    println!("\n{}", loaded.provenance.caveat());
    Ok(())
}
