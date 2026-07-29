//! How much use does the semantic categorizer need before it earns its keep?
//!
//! ```text
//! cargo run -p finsight-eval --bin learning_curve
//! ```
//!
//! # The question this answers
//!
//! `private_eval` measures the centroid pass against a user's accumulated
//! `source='user'` corrections. That raises an obvious product objection: if
//! the thing only works after someone has logged thousands of transactions,
//! it is useless exactly when a new user is forming an opinion of the app.
//!
//! This sweeps the number of accumulated corrections against a FIXED
//! merchant-disjoint holdout and prints the resulting curve, so "when does it
//! start helping" is a measured answer rather than a guess.
//!
//! # Two accumulation orders, because the realistic one is not the flattering one
//!
//! `eval/CATEGORIZATION_CORPUS.md` warns that real corrections are **not a
//! random sample**: a user only corrects what the deterministic passes got
//! wrong or left uncategorized. Sweeping a uniformly-shuffled prefix would
//! model a world where users helpfully label a representative cross-section,
//! which is not the world.
//!
//! So both are reported:
//!
//! - **uniform** — corrections arrive in shuffled order. The optimistic,
//!   unrealistic bound.
//! - **realistic** — only rows the `builtin` keyword pass abstained on or got
//!   wrong are eligible to become corrections, in shuffled order among
//!   themselves. This is the population a review queue actually surfaces.
//!
//! The gap between the two curves IS the accumulation-skew effect the corpus
//! doc says must be accounted for rather than ignored.
//!
//! # What it is not
//!
//! Still the synthetic corpus, so every figure carries that provenance caveat.
//! The SHAPE of the curve (where the knee is, how the two orders differ) is
//! more transportable than its absolute height, which is what this is for.

use anyhow::{Context, Result};
use finsight_eval::categorization::{
    centroid_predictor::{build_prototypes, predict_centroid, Prototype},
    confusion::ConfusionMatrix,
    corpus::{load_corpus_jsonl, LabeledExample},
    predictors::{predict_builtin_for, Prediction},
    split::merchant_disjoint_split,
};
use std::collections::BTreeMap;

const MIN_SCORE: f32 = 0.35;
const HOLDOUT_FRACTION: f64 = 0.3;
const SPLIT_SEED: u64 = 42;

/// Correction counts to sample the curve at. Dense at the low end because that
/// is where the product question lives — nobody wonders whether this works at
/// n=1500.
const STEPS: &[usize] = &[0, 10, 25, 50, 100, 200, 400, 800, 1600];

/// Deterministic shuffle so the curve is reproducible run to run. A random
/// order would make the knee move between runs and there would be no way to
/// tell a real change from noise.
fn shuffled(mut items: Vec<LabeledExample>, seed: u64) -> Vec<LabeledExample> {
    // xorshift64*, inline to avoid pulling a rand dependency into a bin that
    // needs exactly one deterministic permutation.
    let mut state = seed.wrapping_mul(0x2545_F491_4F6C_DD1D) | 1;
    let mut next = || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        state.wrapping_mul(0x2545_F491_4F6C_DD1D)
    };
    for i in (1..items.len()).rev() {
        let j = (next() % (i as u64 + 1)) as usize;
        items.swap(i, j);
    }
    items
}

#[tokio::main]
async fn main() -> Result<()> {
    let corpus_path = std::env::args().nth(1).unwrap_or_else(|| {
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../eval/categorization_corpus.synthetic_multi_archetype.jsonl")
            .to_string()
    });
    let loaded = load_corpus_jsonl(&corpus_path).with_context(|| format!("loading {corpus_path}"))?;

    // The holdout is fixed across every step, or the curve would be comparing
    // different populations at each point and its shape would mean nothing.
    let (reference_pool, holdout) =
        merchant_disjoint_split(&loaded.examples, HOLDOUT_FRACTION, SPLIT_SEED);

    println!("corpus:      {corpus_path}");
    println!("provenance:  {}", loaded.provenance.as_str());
    println!(
        "holdout:     {} rows (fixed, merchant-disjoint), reference pool {}",
        holdout.len(),
        reference_pool.len()
    );

    let data_dir = std::env::var("FINSIGHT_DATA_DIR").unwrap_or_else(|_| "./data".into());
    eprintln!("loading encoder…");
    let encoder = finsight_agent::embedding::get_encoder(std::path::Path::new(&data_dir)).await?;
    println!("encoder:     {} ({} dims)", encoder.model_id(), encoder.dims());

    // Embed once, reuse everywhere — the curve re-slices the same vectors.
    let mut all: Vec<LabeledExample> = reference_pool.clone();
    all.extend(holdout.iter().cloned());
    let all_texts: Vec<String> = all.iter().map(|e| e.merchant_text.clone()).collect();
    let all_vectors = encoder.embed(&all_texts).await?;
    let vec_by_id: BTreeMap<String, Vec<f32>> = all
        .iter()
        .map(|e| e.id.clone())
        .zip(all_vectors.into_iter())
        .collect();

    // The realistic pool: only what a review queue would ever put in front of
    // a user — rows `builtin` abstained on or got wrong. Everything else was
    // already right, so nobody corrects it and it never becomes a label.
    let realistic_pool: Vec<LabeledExample> = reference_pool
        .iter()
        .filter(|ex| {
            let p = predict_builtin_for(ex);
            match p.category {
                None => true,                 // abstained → lands in review
                Some(got) => got != ex.category, // wrong → user corrects it
            }
        })
        .cloned()
        .collect();

    println!(
        "\nreference pool: {} uniform / {} realistic (builtin missed or abstained)",
        reference_pool.len(),
        realistic_pool.len()
    );

    let builtin = ConfusionMatrix::build("builtin", &holdout, predict_builtin_for);
    println!(
        "\nbuiltin (flat reference): coverage {:.1}%  precision {}",
        builtin.coverage() * 100.0,
        builtin.precision().map(|p| format!("{:.1}%", p * 100.0)).unwrap_or("n/a".into())
    );

    for (label, pool) in [("uniform", &reference_pool), ("realistic", &realistic_pool)] {
        let ordered = shuffled(pool.clone(), 1234);
        println!("\n{label} accumulation");
        println!("{:>10} {:>10} {:>10} {:>12}", "corrections", "coverage", "precision", "categories");
        for &n in STEPS {
            if n > ordered.len() {
                continue;
            }
            let prefix = &ordered[..n];
            let vectors: Vec<Vec<f32>> = prefix
                .iter()
                .map(|e| vec_by_id.get(&e.id).cloned().unwrap_or_default())
                .collect();
            let prototypes: Vec<Prototype> = build_prototypes(prefix, &vectors);
            let m = ConfusionMatrix::build("centroid", &holdout, |ex| {
                match vec_by_id.get(&ex.id) {
                    Some(v) => predict_centroid(v, &prototypes, MIN_SCORE),
                    None => Prediction::abstain(),
                }
            });
            println!(
                "{n:>10} {:>9.1}% {:>10} {:>12}",
                m.coverage() * 100.0,
                m.precision().map(|p| format!("{:.1}%", p * 100.0)).unwrap_or("n/a".into()),
                prototypes.len(),
            );
        }
    }

    println!("\n{}", loaded.provenance.caveat());
    Ok(())
}
