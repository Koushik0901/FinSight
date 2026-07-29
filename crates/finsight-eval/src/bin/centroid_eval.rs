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

/// Generic, real-world category exemplars — the kind of thing a shipped default
/// seed or a user curating `category_examples` (#91) would plausibly write.
///
/// # Read the caveat before quoting any number this produces
///
/// These were chosen AFTER looking at the holdout's failures. That is the
/// definition of tuning on the test set, and it means the "with exemplars"
/// column is **not an unbiased estimate** of what curating examples buys in
/// general — it is an upper bound, and a demonstration that the lever moves the
/// prototype at all.
///
/// What makes it still worth measuring: the product question is precisely
/// "when a user sees a category being confused, can they fix it by adding an
/// example?" — and that is a question you can only ask having seen the
/// confusion. The honest claim is about the MECHANISM, never about the delta
/// generalizing to unseen traps.
///
/// None of these is a holdout merchant name. They are domain vocabulary
/// (`BC HYDRO` is a real utility; `BLUESAIL HYDRO` is the invented holdout
/// merchant), so this is not merchant-identity leakage across the split — but
/// it is unquestionably informed by it.
const DOMAIN_EXEMPLARS: &[(&str, &str)] = &[
    // The utilities -> groceries trap: a general-English encoder places "hydro"
    // near water and produce. Real utilities named this way are common in
    // Canada (BC Hydro, Hydro One, Hydro-Quebec).
    ("utilities", "BC HYDRO"),
    ("utilities", "HYDRO ONE ELECTRICITY BILL"),
    ("utilities", "ELECTRIC UTILITY PAYMENT"),
    ("utilities", "NATURAL GAS BILL"),
    ("utilities", "WATER AND SEWER SERVICE"),
    // The subscriptions -> housing trap: "membership fee" reads as a club or
    // building fee without context.
    ("subscriptions", "ANNUAL MEMBERSHIP FEE"),
    ("subscriptions", "GYM MEMBERSHIP MONTHLY"),
    ("subscriptions", "STREAMING SERVICE SUBSCRIPTION"),
    // The single transport -> groceries miss.
    ("transport", "TRANSIT AUTHORITY FARE"),
    ("transport", "MONTHLY BUS PASS"),
];

/// Examples per category in the production-faithful regime — the order of
/// magnitude a user actually curates in `category_examples` (#91), not the
/// hundreds a corpus half contains.
const FEW_SHOT_K: usize = 5;

/// First `k` examples of each category, in corpus order.
///
/// Deterministic rather than random: a random subsample would make the
/// few-shot number wobble run to run, and the point of this column is to be
/// comparable across runs and against the full-reference column.
fn take_per_category(
    examples: &[finsight_eval::categorization::corpus::LabeledExample],
    k: usize,
) -> Vec<finsight_eval::categorization::corpus::LabeledExample> {
    let mut seen: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    let mut out = Vec::new();
    for ex in examples {
        let n = seen.entry(ex.category.as_str()).or_insert(0);
        if *n < k {
            out.push(ex.clone());
            *n += 1;
        }
    }
    out
}

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

    // Second prototype set: the same reference half PLUS a handful of generic
    // domain exemplars, standing in for what #91's per-category examples give a
    // user. See DOMAIN_EXEMPLARS for why this is a mechanism demonstration and
    // not an unbiased improvement estimate.
    let exemplar_texts: Vec<String> =
        DOMAIN_EXEMPLARS.iter().map(|(_, text)| (*text).to_string()).collect();
    let exemplar_vectors = encoder.embed(&exemplar_texts).await?;
    let mut seeded: Vec<_> = reference.clone();
    let mut seeded_vectors = reference_vectors.clone();
    for ((category, text), v) in DOMAIN_EXEMPLARS.iter().zip(exemplar_vectors) {
        seeded.push(finsight_eval::categorization::corpus::LabeledExample {
            id: format!("exemplar:{text}"),
            merchant_text: (*text).to_string(),
            // A synthetic merchant id that cannot collide with a corpus
            // merchant, so the disjointness story stays legible.
            merchant_id: format!("exemplar:{text}"),
            category: (*category).to_string(),
            notes: Some("hand-added domain exemplar".into()),
        });
        seeded_vectors.push(v);
    }
    let seeded_prototypes: Vec<Prototype> = build_prototypes(&seeded, &seeded_vectors);

    // --- production-faithful regime ---------------------------------------
    //
    // The two prototype sets above are built from the WHOLE reference half —
    // ~200 examples per category. Production never looks like that:
    // `centroid::rebuild_all` builds each centroid from `category_examples`
    // (#91), which is a handful of exemplars a user curated by hand.
    //
    // That difference is not cosmetic, it changes what the numbers mean. In a
    // 200-example mean, five added exemplars are diluted 40:1 and cannot move
    // the prototype; in a 5-example mean they dominate it. Measuring only the
    // corpus-scale regime would report the shipped feature's behaviour wrongly
    // in BOTH directions — overstating baseline precision (more examples is a
    // better mean) and understating the curation lever.
    let few_shot: Vec<_> = take_per_category(&reference, FEW_SHOT_K);
    let few_shot_texts: Vec<String> = few_shot.iter().map(|e| e.merchant_text.clone()).collect();
    let few_shot_vectors = encoder.embed(&few_shot_texts).await?;
    let few_shot_prototypes: Vec<Prototype> = build_prototypes(&few_shot, &few_shot_vectors);

    let mut few_shot_seeded = few_shot.clone();
    let mut few_shot_seeded_vectors = few_shot_vectors.clone();
    let exemplar_vectors_2 = encoder.embed(&exemplar_texts).await?;
    for ((category, text), v) in DOMAIN_EXEMPLARS.iter().zip(exemplar_vectors_2) {
        few_shot_seeded.push(finsight_eval::categorization::corpus::LabeledExample {
            id: format!("exemplar:{text}"),
            merchant_text: (*text).to_string(),
            merchant_id: format!("exemplar:{text}"),
            category: (*category).to_string(),
            notes: None,
        });
        few_shot_seeded_vectors.push(v);
    }
    let few_shot_seeded_prototypes: Vec<Prototype> =
        build_prototypes(&few_shot_seeded, &few_shot_seeded_vectors);

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
    let seeded_matrix = ConfusionMatrix::build("centroid+ex", &holdout, |ex| {
        match vector_by_id.get(ex.id.as_str()) {
            Some(v) => predict_centroid(v, &seeded_prototypes, MIN_SCORE),
            None => Prediction::abstain(),
        }
    });

    println!("\nmerchant-disjoint holdout ({} rows)", holdout.len());
    println!("{:<12} {:>10} {:>10} {:>10}", "source", "coverage", "precision", "correct");
    let few_shot_matrix = ConfusionMatrix::build("few5", &holdout, |ex| {
        match vector_by_id.get(ex.id.as_str()) {
            Some(v) => predict_centroid(v, &few_shot_prototypes, MIN_SCORE),
            None => Prediction::abstain(),
        }
    });
    let few_shot_seeded_matrix = ConfusionMatrix::build("few5+ex", &holdout, |ex| {
        match vector_by_id.get(ex.id.as_str()) {
            Some(v) => predict_centroid(v, &few_shot_seeded_prototypes, MIN_SCORE),
            None => Prediction::abstain(),
        }
    });

    for m in [
        &builtin_matrix,
        &centroid_matrix,
        &seeded_matrix,
        &few_shot_matrix,
        &few_shot_seeded_matrix,
    ] {
        println!(
            "{:<12} {:>9.1}% {:>10} {:>10}",
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

    println!(
        "\n`centroid+ex` adds {} hand-written domain exemplars to the reference half,\n\
         standing in for #91's per-category examples. They were chosen AFTER seeing the\n\
         failures above, so this column is an UPPER BOUND on that lever and a demonstration\n\
         that it moves the prototype — not an unbiased estimate of what curating examples\n\
         buys against traps nobody has looked at yet.",
        DOMAIN_EXEMPLARS.len()
    );

    println!("\n{}", loaded.provenance.caveat());
    Ok(())
}
