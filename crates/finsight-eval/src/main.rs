//! FinSight Copilot evaluation harness.
//!
//! Drives the real `ReasoningEngine` + tool loop over a deterministic synthetic
//! household (see `seed.rs`) for every question in a benchmark file, and writes
//! one JSON line of results per question for an external judge to score.
//!
//! Each question runs against its own freshly-seeded database so questions never
//! interfere (e.g. a recategorization draft in one cannot leak into the next).
//!
//! Key resolution: `OPENROUTER_API_KEY` env var first, then the OS keychain slot
//! the app itself uses (`com.finsight.llm` / `openrouter`). The key is never
//! printed. This is a dev/CI tool, not the shipped app, so reading the env var
//! here does not violate the app's keychain-only key policy.
//!
//! Usage:
//!   cargo run -p finsight-eval --release -- \
//!     --benchmark eval/benchmark.jsonl --out eval/runs/<ts>/copilot_outputs.jsonl \
//!     [--model z-ai/glm-5.2:exacto] [--limit N] [--timeout-secs 180]

mod seed;

use std::io::{BufRead, Write};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use clap::Parser;
use finsight_agent::providers::openai_compat::OpenAiCompatProvider;
use finsight_agent::reasoning::engine::ReasoningEngine;
use finsight_agent::reasoning::tools::standard_toolset;
use finsight_agent::CompletionProvider;
use finsight_core::{db::run_migrations, keychain, Db};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;

#[derive(Parser, Debug)]
#[command(about = "Run the FinSight Copilot benchmark and emit per-question results as JSONL")]
struct Args {
    /// Benchmark file: one JSON object per line, each with at least `id` and `question`.
    #[arg(long)]
    benchmark: String,
    /// Output JSONL path (created/truncated). Parent dirs must exist.
    #[arg(long)]
    out: String,
    /// OpenRouter model id for the Copilot under test.
    #[arg(long, default_value = "z-ai/glm-5.2:exacto")]
    model: String,
    /// OpenAI-compatible base URL.
    #[arg(long, default_value = "https://openrouter.ai/api/v1")]
    base_url: String,
    /// Only run the first N questions (smoke tests). 0 = all.
    #[arg(long, default_value_t = 0)]
    limit: usize,
    /// Per-question wall-clock ceiling for the whole tool loop, in seconds.
    #[arg(long, default_value_t = 180)]
    timeout_secs: u64,
    /// Max reasoning-engine tool-loop iterations per question.
    #[arg(long, default_value_t = 10)]
    max_iterations: usize,
}

/// One benchmark question (only `id`/`question` are required; the rest is passed
/// through untouched for the judge).
#[derive(Debug, Deserialize)]
struct BenchQuestion {
    id: String,
    question: String,
    #[serde(flatten)]
    rest: serde_json::Map<String, Value>,
}

#[derive(Debug, Serialize)]
struct DraftActionOut {
    action_kind: String,
    rationale: String,
    confidence: f64,
}

/// One line of harness output — everything the judge needs plus run metadata.
#[derive(Debug, Serialize)]
struct ResultOut {
    id: String,
    question: String,
    model: String,
    /// Passthrough of the benchmark row's other fields (category, difficulty,
    /// reference_facts, expected_tools, notes, …) so the judge sees them.
    #[serde(flatten)]
    bench: serde_json::Map<String, Value>,
    // ── Copilot answer ──
    answer: String,
    reasoning: String,
    plan: Vec<String>,
    /// Human-readable tool trace ("Called tool: X", "Tool result: …").
    trace: Vec<String>,
    /// Just the tool names that were called, extracted from the trace.
    tools_called: Vec<String>,
    response_blocks: Vec<Value>,
    response_block_kinds: Vec<String>,
    draft_actions: Vec<DraftActionOut>,
    follow_up_questions: Vec<String>,
    assumptions: Vec<String>,
    data_sources: Vec<String>,
    missing_data: Vec<String>,
    // ── Run metadata ──
    latency_ms: u128,
    /// Mirrors `is_usable_tool_answer`: used a tool AND produced non-empty prose.
    /// A `false` here in production would have been replaced by the planner
    /// fallback, so it is a first-class failure signal.
    is_usable: bool,
    error: Option<String>,
    seed_as_of: String,
}

fn resolve_key() -> Result<String> {
    if let Ok(k) = std::env::var("OPENROUTER_API_KEY") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Ok(k);
        }
    }
    if let Ok(Some(k)) = keychain::get_key("com.finsight.llm", "openrouter") {
        let k = k.trim().to_string();
        if !k.is_empty() {
            return Ok(k);
        }
    }
    Err(anyhow!(
        "No OpenRouter key found. Set OPENROUTER_API_KEY, or save your key in the app \
         (Settings → Agent) so it lands in the keychain slot com.finsight.llm/openrouter."
    ))
}

fn tool_names_from_trace(trace: &[String]) -> Vec<String> {
    trace
        .iter()
        .filter_map(|t| t.strip_prefix("Called tool: ").map(|s| s.trim().to_string()))
        .collect()
}

fn block_kind(b: &Value) -> String {
    b.get("kind").and_then(|k| k.as_str()).unwrap_or("unknown").to_string()
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    let key = resolve_key()?;
    let seed_as_of = chrono::Utc::now().date_naive().to_string();

    let questions: Vec<BenchQuestion> = {
        let file = std::fs::File::open(&args.benchmark)
            .with_context(|| format!("opening benchmark {}", args.benchmark))?;
        let mut out = Vec::new();
        for (i, line) in std::io::BufReader::new(file).lines().enumerate() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let q: BenchQuestion = serde_json::from_str(line)
                .with_context(|| format!("parsing benchmark line {}", i + 1))?;
            out.push(q);
        }
        out
    };
    let total = if args.limit > 0 {
        args.limit.min(questions.len())
    } else {
        questions.len()
    };

    let mut out_file = std::fs::File::create(&args.out)
        .with_context(|| format!("creating output {}", args.out))?;

    eprintln!(
        "▶ eval: {total} question(s) · model={} · base={} · timeout={}s",
        args.model, args.base_url, args.timeout_secs
    );

    for (idx, q) in questions.into_iter().take(total).enumerate() {
        eprint!("  [{}/{}] {} … ", idx + 1, total, q.id);
        let _ = std::io::stderr().flush();

        // Fresh, isolated, seeded DB per question.
        let dir = TempDir::new()?;
        let dbkey = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("eval.sqlcipher"), &dbkey)?;
        run_migrations(&db)?;
        {
            let mut conn = db.get()?;
            seed::seed(&mut conn);
        }

        let provider: Arc<dyn CompletionProvider> = Arc::new(OpenAiCompatProvider::new(
            args.base_url.as_str(),
            key.clone(),
            args.model.as_str(),
            "openrouter",
        ));
        let tools = standard_toolset();

        let started = Instant::now();
        let mut conn = db.get()?;
        let run = ReasoningEngine::run(&mut conn, &q.question, &tools, provider, args.max_iterations);
        let outcome = tokio::time::timeout(Duration::from_secs(args.timeout_secs), run).await;
        let latency_ms = started.elapsed().as_millis();

        let record = match outcome {
            Err(_) => make_error_record(&q, &args.model, &seed_as_of, latency_ms, "timed out"),
            Ok(Err(e)) => {
                make_error_record(&q, &args.model, &seed_as_of, latency_ms, &e.to_string())
            }
            Ok(Ok(r)) => {
                let tools_called = tool_names_from_trace(&r.trace);
                let is_usable = !tools_called.is_empty() && !r.content.trim().is_empty();
                let response_block_kinds = r.response_blocks.iter().map(block_kind).collect();
                ResultOut {
                    id: q.id.clone(),
                    question: q.question.clone(),
                    model: args.model.clone(),
                    bench: q.rest.clone(),
                    answer: r.content,
                    reasoning: r.reasoning,
                    plan: r.plan,
                    trace: r.trace,
                    tools_called,
                    response_block_kinds,
                    response_blocks: r.response_blocks,
                    draft_actions: r
                        .draft_actions
                        .into_iter()
                        .map(|d| DraftActionOut {
                            action_kind: d.action_kind,
                            rationale: d.rationale,
                            confidence: d.confidence,
                        })
                        .collect(),
                    follow_up_questions: r.follow_up_questions,
                    assumptions: r.assumptions,
                    data_sources: r.data_sources,
                    missing_data: r.missing_data,
                    latency_ms,
                    is_usable,
                    error: None,
                    seed_as_of: seed_as_of.clone(),
                }
            }
        };

        writeln!(out_file, "{}", serde_json::to_string(&record)?)?;
        out_file.flush()?;
        match &record.error {
            Some(e) => eprintln!("ERROR ({e}) [{}ms]", latency_ms),
            None => eprintln!(
                "ok · tools={:?} · usable={} [{}ms]",
                record.tools_called, record.is_usable, latency_ms
            ),
        }
    }

    eprintln!("✔ wrote {}", args.out);
    Ok(())
}

fn make_error_record(
    q: &BenchQuestion,
    model: &str,
    seed_as_of: &str,
    latency_ms: u128,
    err: &str,
) -> ResultOut {
    ResultOut {
        id: q.id.clone(),
        question: q.question.clone(),
        model: model.to_string(),
        bench: q.rest.clone(),
        answer: String::new(),
        reasoning: String::new(),
        plan: Vec::new(),
        trace: Vec::new(),
        tools_called: Vec::new(),
        response_blocks: Vec::new(),
        response_block_kinds: Vec::new(),
        draft_actions: Vec::new(),
        follow_up_questions: Vec::new(),
        assumptions: Vec::new(),
        data_sources: Vec::new(),
        missing_data: Vec::new(),
        latency_ms,
        is_usable: false,
        error: Some(err.to_string()),
        seed_as_of: seed_as_of.to_string(),
    }
}
