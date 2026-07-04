//! Live #3 verification: run the real LLM categorizer (Gemma via OpenRouter) on
//! the real dev-app data's uncategorized transactions. Ignored; run manually
//! with the app stopped (writes to the dev DB):
//!   OPENROUTER_API_KEY=... cargo test -p finsight-app --test phase6_categorize_live -- --ignored --nocapture

use std::sync::Arc;

use finsight_agent::agent::{AgentEvent, AgentJob, EventCallback};
use finsight_agent::categorizer;
use finsight_agent::providers::openai_compat::OpenAiCompatProvider;
use finsight_agent::CompletionProvider;
use finsight_core::{keychain, Db};

fn open() -> Db {
    let appdata = std::env::var("APPDATA").expect("APPDATA");
    let db_path = std::path::Path::new(&appdata)
        .join("com.finsight.app")
        .join("data.sqlcipher");
    let key = keychain::load_or_create_key("com.finsight.app", "default").expect("db key");
    Db::open(&db_path, &key).expect("open dev db")
}

fn uncategorized_count(db: &Db) -> i64 {
    let conn = db.get().unwrap();
    conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND is_transfer = 0",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "hits OpenRouter and writes to the dev DB; run manually"]
async fn categorize_real_uncategorized_with_gemma() {
    let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
    let provider: Arc<dyn CompletionProvider> = Arc::new(OpenAiCompatProvider::new(
        "https://openrouter.ai/api/v1",
        key,
        "google/gemma-4-31b-it",
        "openrouter",
    ));

    let db = open();
    let before = uncategorized_count(&db);
    println!("uncategorized before: {before}");

    let cb: EventCallback = Arc::new(|e: AgentEvent| {
        if let AgentEvent::CategorizationProgress { .. } = &e {
            // progress noise suppressed
        }
    });
    categorizer::run_job(&db, AgentJob::CategorizeAll, provider, cb)
        .await
        .expect("categorize job");

    let after = uncategorized_count(&db);
    println!("uncategorized after:  {after}");

    let conn = db.get().unwrap();
    let llm_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM categorizations WHERE source = 'llm'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let needs_review: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM transactions WHERE ai_confidence IS NOT NULL AND ai_confidence < 0.6",
            [],
            |r| r.get(0),
        )
        .unwrap();
    println!("llm categorizations: {llm_count}, needs-review (low-confidence): {needs_review}");

    // The LLM should have categorized a meaningful chunk of the local merchants.
    assert!(after < before, "categorization should reduce the uncategorized count");
    assert!(llm_count > 0, "LLM categorizations should now exist");
}
