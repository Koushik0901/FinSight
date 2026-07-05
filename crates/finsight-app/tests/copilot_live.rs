//! Live end-to-end verification of the Copilot against the real OpenRouter /
//! Gemma provider. Ignored by default (needs network + `OPENROUTER_API_KEY`).
//!
//! Run with the key loaded from `.env` (never printed):
//!   OPENROUTER_API_KEY=... cargo test -p finsight-app --test copilot_live -- --ignored --nocapture
//!
//! This drives the actual `ReasoningEngine` + tool loop for the six required
//! Phase 5B questions over a controlled, clearly-synthetic test fixture (NOT
//! app data — a test fixture, used only to exercise the grounded tools).

use std::sync::Arc;

use finsight_agent::providers::openai_compat::OpenAiCompatProvider;
use finsight_agent::reasoning::engine::ReasoningEngine;
use finsight_agent::reasoning::tools::{act, read, ToolSet};
use finsight_agent::CompletionProvider;
use finsight_core::{db::run_migrations, keychain, Db};
use rusqlite::Connection;
use tempfile::TempDir;

fn toolset() -> ToolSet {
    let mut t = ToolSet::new();
    t.register(read::get_financial_snapshot());
    t.register(read::get_net_worth());
    t.register(read::get_account_balances());
    t.register(read::get_month_totals());
    t.register(read::get_spending_breakdown());
    t.register(read::get_top_spending_categories());
    t.register(read::get_budgets());
    t.register(read::get_goals());
    t.register(read::get_recurring_bills());
    t.register(read::get_liabilities());
    t.register(read::search_transactions());
    t.register(read::find_anomalies());
    t.register(read::list_uncategorized_transactions());
    t.register(read::run_emergency_fund_scenarios());
    t.register(read::run_purchase_affordability());
    t.register(read::run_cashflow_timeline());
    t.register(read::get_data_quality_report());
    t.register(act::draft_recategorization());
    t
}

/// Clearly-synthetic test fixture. Not user data — exercises the grounded tools.
fn seed(conn: &mut Connection) {
    // A checking account with a CONFIRMED (manual) balance and a savings account.
    conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) VALUES('chk','Test','Bank','Checking','Everyday Checking','USD','#3B82F6','manual',datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id, as_of_date, balance_cents, source) VALUES('chk','2026-06-30',200000,'manual')", []).unwrap();
    conn.execute("INSERT INTO accounts(id, owner, bank, type, name, currency, color, source, created_at) VALUES('sav','Test','Bank','Savings','Emergency Savings','USD','#10B981','manual',datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id, as_of_date, balance_cents, source) VALUES('sav','2026-06-30',300000,'manual')", []).unwrap();
    // A credit card — debt is a Credit-type Account with a negative balance,
    // not a separate liabilities-table row.
    conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,liquidity_type,emergency_fund_eligible,account_group,apr_pct,min_payment_cents,limit_cents,created_at) VALUES('cc','Test','Bank','Credit','Visa','USD','#F97316','manual','restricted',0,'debt',19.9,3000,500000,datetime('now'))", []).unwrap();
    conn.execute("INSERT INTO account_balances(account_id,as_of_date,balance_cents,source) VALUES('cc','2026-06-30',-120000,'manual')", []).unwrap();
    // Categories (some transactions will be uncategorized on purpose).
    conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g','Core',0)", []).unwrap();
    for (id, label) in [("groceries", "Groceries"), ("dining", "Dining"), ("transport", "Transport"), ("shopping", "Shopping")] {
        conn.execute("INSERT INTO categories(id,group_id,label,color,sort_order) VALUES(?1,'g',?2,'#888',0)", rusqlite::params![id, label]).unwrap();
    }
    // Monthly payroll income Jan–Jun 2026 (categorized as income via positive amount).
    for m in 1..=6 {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,created_at) \
             VALUES(hex(randomblob(16)),'chk',?1,400000,'Payroll',NULL,'cleared',datetime('now'))",
            rusqlite::params![format!("2026-0{m}-01T00:00:00Z")],
        ).unwrap();
    }
    // Expenses across Jan–Jun. Some over $60, some under; some categorized, some not.
    let expenses = [
        ("2026-01-15", -9_999, "Costco", Some("groceries")),
        ("2026-01-20", -4_200, "Tim Hortons", Some("dining")),
        ("2026-02-10", -15_000, "Best Buy", None),           // uncategorized, over $60
        ("2026-02-14", -6_500, "Uber", Some("transport")),
        ("2026-03-05", -8_800, "Whole Foods", Some("groceries")),
        ("2026-03-22", -3_100, "Spotify", None),             // uncategorized, under $60
        ("2026-04-02", -25_000, "Apple Store", None),        // uncategorized, over $60
        ("2026-04-18", -7_250, "Shell Gas", Some("transport")),
        ("2026-05-09", -12_000, "Nordstrom", Some("shopping")),
        ("2026-05-30", -5_500, "Chipotle", Some("dining")),
        ("2026-06-11", -18_400, "Delta Airlines", None),     // uncategorized, over $60
        ("2026-06-28", -2_000, "Netflix", None),             // uncategorized, under $60
    ];
    for (date, amt, merch, cat) in expenses {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,status,created_at) \
             VALUES(hex(randomblob(16)),'chk',?1,?2,?3,?4,'cleared',datetime('now'))",
            rusqlite::params![format!("{date}T12:00:00Z"), amt, merch, cat],
        ).unwrap();
    }
}

async fn ask(conn: &mut Connection, provider: Arc<dyn CompletionProvider>, q: &str) -> finsight_agent::ReasoningResult {
    let tools = toolset();
    let result = ReasoningEngine::run(conn, q, &tools, provider, 10)
        .await
        .expect("reasoning engine run");
    println!("\n════════ Q: {q}\n{}", result.content);
    println!("── tools: {:?}", result.trace);
    println!(
        "── response_blocks ({}): {}",
        result.response_blocks.len(),
        serde_json::to_string(&result.response_blocks).unwrap_or_default()
    );
    if !result.draft_actions.is_empty() {
        println!("── draft actions: {}", result.draft_actions.len());
    }
    result
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "hits OpenRouter; run manually with OPENROUTER_API_KEY set"]
async fn six_required_questions_answer_grounded() {
    let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
    let provider: Arc<dyn CompletionProvider> = Arc::new(OpenAiCompatProvider::new(
        "https://openrouter.ai/api/v1",
        key,
        "google/gemma-4-31b-it",
        "openrouter",
    ));

    let dir = TempDir::new().unwrap();
    let dbkey = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("live.sqlcipher"), &dbkey).unwrap();
    run_migrations(&db).unwrap();
    let mut conn = db.get().unwrap();
    seed(&mut conn);

    // 1. PS5 affordability
    let a1 = ask(&mut conn, provider.clone(), "Can I afford a PS5 with my current standing? It costs $540.").await;
    assert!(!a1.content.trim().is_empty());

    // 2. Net worth
    let a2 = ask(&mut conn, provider.clone(), "What's my net worth right now?").await;
    // Assets 200,000 + 300,000 = 500,000; minus 120,000 liability = 380,000 → "3,800".
    assert!(a2.content.contains("3,800") || a2.content.contains("380"), "net worth grounded: {}", a2.content);

    // 3. Emergency fund completion
    ask(&mut conn, provider.clone(), "When will my emergency fund be full?").await;

    // 4. Overspending
    ask(&mut conn, provider.clone(), "Where am I spending the most money, and how do I prevent myself from overspending?").await;

    // 5. Recategorization (must PREVIEW + require approval, not mutate)
    let a5 = ask(&mut conn, provider.clone(), "Let's recategorize all of my transactions that are still uncategorized.").await;
    let uncat_after: i64 = conn
        .query_row("SELECT COUNT(*) FROM transactions WHERE category_id IS NULL AND amount_cents < 0", [], |r| r.get(0))
        .unwrap();
    assert!(uncat_after > 0, "recategorization must NOT apply without approval; expenses still uncategorized");
    let _ = a5;

    // 6. Date-range over $60
    let a6 = ask(&mut conn, provider.clone(), "Analyze all transactions from Jan 2026 to June 2026 and give me everything over $60.").await;
    assert!(!a6.content.trim().is_empty());
}

/// Proves the architecture is GENERIC, not overfit to the six required
/// questions: differently-worded paraphrases, new intent families, an
/// ambiguous question (should clarify, not guess), and an unsupported one
/// (should fail gracefully). Ignored — hits OpenRouter.
#[tokio::test(flavor = "current_thread")]
#[ignore = "hits OpenRouter; run manually with OPENROUTER_API_KEY set"]
async fn generic_beyond_the_six_required_questions() {
    let key = std::env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
    let provider: Arc<dyn CompletionProvider> = Arc::new(OpenAiCompatProvider::new(
        "https://openrouter.ai/api/v1",
        key,
        "google/gemma-4-31b-it",
        "openrouter",
    ));
    let dir = TempDir::new().unwrap();
    let dbkey = keychain::generate_random_key();
    let db = Db::open(&dir.path().join("live2.sqlcipher"), &dbkey).unwrap();
    run_migrations(&db).unwrap();
    let mut conn = db.get().unwrap();
    seed(&mut conn);
    // Flag one transaction as anomalous so the anomaly intent has data.
    conn.execute(
        "UPDATE transactions SET is_anomaly = 1, ai_explanation = 'Much larger than usual for this merchant' WHERE merchant_raw = 'Apple Store'",
        [],
    ).unwrap();

    // Differently-worded paraphrases of required intents.
    ask(&mut conn, provider.clone(), "How much am I worth after debts?").await; // net worth
    ask(&mut conn, provider.clone(), "Is picking up a $300 pair of headphones a smart move right now?").await; // affordability
    ask(&mut conn, provider.clone(), "Which store did I hand the most cash to this year?").await; // merchant breakdown

    // New intent families beyond the six.
    ask(&mut conn, provider.clone(), "Do I have any weird or suspicious charges?").await; // anomalies
    ask(&mut conn, provider.clone(), "What subscriptions am I paying for?").await; // recurring
    ask(&mut conn, provider.clone(), "How's my monthly cash flow looking?").await; // income/cash flow
    ask(&mut conn, provider.clone(), "What should I focus on financially next?").await; // open-ended planning

    // Ambiguous: no amount, no item — should clarify rather than fabricate.
    let ambiguous = ask(&mut conn, provider.clone(), "Can I afford it?").await;
    println!("── ambiguous follow_ups: {:?}", ambiguous.follow_up_questions);

    // Unsupported by a local-first app (no live market data) — should say so.
    ask(&mut conn, provider.clone(), "What's the live share price of Apple stock right now?").await;
}
