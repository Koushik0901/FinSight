//! CI-friendly runner for the categorization precision/coverage harness
//! (issue #88). Loads a labeled corpus (issue #89's format), runs the
//! merchant-disjoint split, computes a confusion matrix + threshold sweep per
//! source, and prints the result as JSON — a human can read it directly, or a
//! CI job can pipe it into a diff/threshold check.
//!
//! Usage:
//!   cargo run -p finsight-eval --bin categorization_eval -- \
//!     --corpus eval/categorization_corpus.synthetic.jsonl \
//!     [--holdout-fraction 0.3] [--seed 42] [--out eval/runs/categorization_baseline.json]
//!
//! See `eval/CATEGORIZATION_CORPUS.md` for the corpus format and how to add
//! new labeled examples.

use clap::Parser;
use finsight_eval::categorization::{corpus, report};

#[derive(Parser, Debug)]
#[command(about = "Run the categorization precision/coverage eval harness and emit a JSON report")]
struct Args {
    /// Labeled corpus in the JSONL format documented in
    /// eval/CATEGORIZATION_CORPUS.md.
    #[arg(long, default_value = "eval/categorization_corpus.synthetic.jsonl")]
    corpus: String,
    /// Target fraction of UNIQUE MERCHANTS placed in the held-out half of the
    /// merchant-disjoint split.
    #[arg(long, default_value_t = 0.3)]
    holdout_fraction: f64,
    /// Seed for the deterministic merchant-disjoint split.
    #[arg(long, default_value_t = 42)]
    seed: u64,
    /// Write the JSON report here instead of stdout.
    #[arg(long)]
    out: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let examples = corpus::load_corpus_jsonl(&args.corpus)?;
    eprintln!("loaded {} labeled examples from {}", examples.len(), args.corpus);

    let stats = corpus::corpus_stats(&examples);
    eprintln!(
        "  {} unique merchants, {} categories",
        stats.unique_merchants,
        stats.category_distribution.len()
    );

    let rep = report::run(&examples, args.holdout_fraction, args.seed);
    let json = serde_json::to_string_pretty(&rep)?;

    match &args.out {
        Some(path) => {
            if let Some(parent) = std::path::Path::new(path).parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(path, &json)?;
            eprintln!("wrote {path}");
        }
        None => println!("{json}"),
    }

    for source in &rep.sources {
        eprintln!(
            "  {:<8} full: precision={:>6} coverage={:.2}  |  holdout: precision={:>6} coverage={:.2}",
            source.source,
            source
                .full_corpus
                .precision
                .map(|p| format!("{:.1}%", p * 100.0))
                .unwrap_or_else(|| "n/a".to_string()),
            source.full_corpus.coverage,
            source
                .holdout_only
                .precision
                .map(|p| format!("{:.1}%", p * 100.0))
                .unwrap_or_else(|| "n/a".to_string()),
            source.holdout_only.coverage,
        );
    }
    eprintln!("✔ done ({})", rep.caveat);

    Ok(())
}
