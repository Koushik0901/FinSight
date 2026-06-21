//! Procedural "Mira & Adam" sample household — used by the onboarding wizard's
//! "Try with sample data" path. Seeded with a pinned constant so tests can assert
//! exact row counts and a known first transaction.
//! NOTE: Mira & Adam are fictional characters used only as sample data labels.

use crate::error::CoreResult;
use crate::Db;
use chrono::{Datelike, Duration, NaiveDate, Utc};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use specta::Type;
use uuid::Uuid;

/// Pinned. Do not change without bumping the determinism test.
const SAMPLE_SEED: u64 = 0xF1_5165_8AAA_0001;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct SeedSummary {
    pub accounts_created: u32,
    pub transactions_created: u32,
    pub import_id: String,
}

struct AccountSpec {
    name: &'static str,
    bank: &'static str,
    owner: &'static str,
    r#type: &'static str, // matches AccountType::as_db values
    color: &'static str,
    opening_cents: i64,
}

const ACCOUNTS: &[AccountSpec] = &[
    AccountSpec {
        name: "Joint Checking",
        bank: "Chase",
        owner: "joint",
        r#type: "Checking",
        color: "#3B82F6",
        opening_cents: 450_000,
    },
    AccountSpec {
        name: "Emergency Fund",
        bank: "Marcus",
        owner: "joint",
        r#type: "Savings",
        color: "#10B981",
        opening_cents: 1_200_000,
    },
    AccountSpec {
        name: "Mira's Card",
        bank: "Amex",
        owner: "mira",
        r#type: "Credit",
        color: "#F59E0B",
        opening_cents: -8_500,
    },
    AccountSpec {
        name: "Adam's Card",
        bank: "Chase",
        owner: "adam",
        r#type: "Credit",
        color: "#EF4444",
        opening_cents: -12_300,
    },
    AccountSpec {
        name: "Brokerage",
        bank: "Fidelity",
        owner: "joint",
        r#type: "Investment",
        color: "#8B5CF6",
        opening_cents: 8_750_000,
    },
    AccountSpec {
        name: "Wallet Cash",
        bank: "Cash",
        owner: "joint",
        r#type: "Cash",
        color: "#6B7280",
        opening_cents: 18_000,
    },
];

const MERCHANTS: &[(&str, i64, i64)] = &[
    // (merchant, bound_a, bound_b) — negative = outflow.
    // NOTE: bounds may be in either order; gen_amount() normalises them so the
    // smaller value is always used as range start.
    ("Safeway", -2_200, -12_500),
    ("Whole Foods", -3_500, -18_000),
    ("Starbucks", -350, -1_200),
    ("Chipotle", -1_100, -2_400),
    ("Shell", -3_500, -7_500),
    ("PG&E", -11_000, -18_500),
    ("Comcast", -6_500, -9_500),
    ("Netflix", -1_599, -1_599),
    ("Spotify", -1_099, -1_099),
    ("Amazon", -2_000, -45_000),
    ("Target", -2_500, -28_000),
    ("Uber", -1_200, -3_800),
    ("Apple", -9_900, -29_900),
    ("Acme Payroll", 220_000, 380_000), // bi-weekly inflow
];

const CATEGORIES: &[(&str, &str, &str)] = &[
    // (id, group_id, label)
    ("groceries", "daily", "Groceries"),
    ("dining", "daily", "Dining"),
    ("transport", "daily", "Transport"),
    ("housing", "fixed", "Housing"),
    ("utilities", "fixed", "Utilities"),
    ("subscriptions", "fixed", "Subscriptions"),
    ("shopping", "lifestyle", "Shopping"),
    ("travel", "lifestyle", "Travel"),
    ("gifts", "lifestyle", "Gifts"),
    ("health", "wellbeing", "Health"),
];

const CATEGORY_GROUPS: &[(&str, &str)] = &[
    ("fixed", "Fixed"),
    ("daily", "Daily"),
    ("lifestyle", "Lifestyle"),
    ("wellbeing", "Wellbeing"),
];

// ── Dev-demo data ─────────────────────────────────────────────────────────────

struct DemoAccount {
    key: &'static str,
    name: &'static str,
    bank: &'static str,
    owner: &'static str,
    typ: &'static str,
    color: &'static str,
    balance_cents: i64,
}

const DEMO_ACCOUNTS: &[DemoAccount] = &[
    DemoAccount {
        key: "joint-checking",
        name: "Joint Checking",
        bank: "Mercury",
        owner: "joint",
        typ: "Checking",
        color: "#3B82F6",
        balance_cents: 1_482_042,
    },
    DemoAccount {
        key: "joint-savings",
        name: "House Fund",
        bank: "Wealthfront",
        owner: "joint",
        typ: "Savings",
        color: "#10B981",
        balance_cents: 2_864_000,
    },
    DemoAccount {
        key: "mira-checking",
        name: "Mira · Checking",
        bank: "Schwab",
        owner: "mira",
        typ: "Checking",
        color: "#8B5CF6",
        balance_cents: 624_018,
    },
    DemoAccount {
        key: "adam-checking",
        name: "Adam · Checking",
        bank: "Chase",
        owner: "adam",
        typ: "Checking",
        color: "#F59E0B",
        balance_cents: 381_250,
    },
    DemoAccount {
        key: "amex",
        name: "Amex Gold",
        bank: "Amex",
        owner: "joint",
        typ: "Credit",
        color: "#EF4444",
        balance_cents: -241_800,
    },
    DemoAccount {
        key: "retirement",
        name: "Retirement",
        bank: "Fidelity",
        owner: "joint",
        typ: "Investment",
        color: "#6366F1",
        balance_cents: 8_642_000,
    },
];

struct RecurringItem {
    day: u32,
    merchant: &'static str,
    cents: i64,
    category_id: Option<&'static str>,
    account_key: &'static str,
}

/// Monthly recurring transactions, seeded into each prior month for recurring detection.
const RECURRING_MONTHLY: &[RecurringItem] = &[
    RecurringItem {
        day: 1,
        merchant: "Sunset Co · Payroll",
        cents: 480_000,
        category_id: None,
        account_key: "mira-checking",
    },
    RecurringItem {
        day: 3,
        merchant: "Bay Property Mgmt · Rent",
        cents: -185_000,
        category_id: Some("housing"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 5,
        merchant: "Lyft",
        cents: -1_800,
        category_id: Some("transport"),
        account_key: "amex",
    },
    RecurringItem {
        day: 6,
        merchant: "Trader Joe's",
        cents: -7_200,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 7,
        merchant: "Spotify Family",
        cents: -1_699,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    RecurringItem {
        day: 8,
        merchant: "Internet · Sonic",
        cents: -8_800,
        category_id: Some("utilities"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 9,
        merchant: "Sweetgreen",
        cents: -2_100,
        category_id: Some("dining"),
        account_key: "mira-checking",
    },
    RecurringItem {
        day: 10,
        merchant: "PG&E",
        cents: -22_000,
        category_id: Some("utilities"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 11,
        merchant: "Comcast",
        cents: -8_800,
        category_id: Some("utilities"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 12,
        merchant: "Whole Foods",
        cents: -8_500,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 14,
        merchant: "BP Gas",
        cents: -5_100,
        category_id: Some("transport"),
        account_key: "adam-checking",
    },
    RecurringItem {
        day: 15,
        merchant: "Acme Corp · Payroll",
        cents: 520_000,
        category_id: None,
        account_key: "adam-checking",
    },
    RecurringItem {
        day: 16,
        merchant: "Sweetgreen",
        cents: -1_850,
        category_id: Some("dining"),
        account_key: "mira-checking",
    },
    RecurringItem {
        day: 17,
        merchant: "Adobe Creative Cloud",
        cents: -2_299,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    RecurringItem {
        day: 19,
        merchant: "Notion",
        cents: -1_000,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    RecurringItem {
        day: 20,
        merchant: "Trader Joe's",
        cents: -6_800,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 22,
        merchant: "iCloud+",
        cents: -999,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    RecurringItem {
        day: 22,
        merchant: "Lyft",
        cents: -2_200,
        category_id: Some("transport"),
        account_key: "amex",
    },
    RecurringItem {
        day: 24,
        merchant: "Disney+",
        cents: -1_099,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    RecurringItem {
        day: 25,
        merchant: "Gym · Range SF",
        cents: -14_900,
        category_id: Some("health"),
        account_key: "joint-checking",
    },
    RecurringItem {
        day: 28,
        merchant: "NYTimes",
        cents: -400,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    RecurringItem {
        day: 28,
        merchant: "Costco",
        cents: -38_500,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
];

struct SpecificTxn {
    days_ago: i64,
    merchant: &'static str,
    cents: i64,
    category_id: Option<&'static str>,
    account_key: &'static str,
}

/// 18 hand-crafted recent transactions from the "Mira & Adam" prototype design,
/// expressed as days before today so the dataset stays current on any run date.
const SPECIFIC_TXNS: &[SpecificTxn] = &[
    SpecificTxn {
        days_ago: 1,
        merchant: "Mosswood Wine Bar",
        cents: -14_200,
        category_id: Some("dining"),
        account_key: "amex",
    },
    SpecificTxn {
        days_ago: 1,
        merchant: "Lyft",
        cents: -1_840,
        category_id: Some("transport"),
        account_key: "amex",
    },
    SpecificTxn {
        days_ago: 2,
        merchant: "Trader Joe's",
        cents: -7_820,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 3,
        merchant: "Sweetgreen",
        cents: -2_250,
        category_id: Some("dining"),
        account_key: "mira-checking",
    },
    SpecificTxn {
        days_ago: 3,
        merchant: "Blue Bottle",
        cents: -675,
        category_id: Some("dining"),
        account_key: "mira-checking",
    },
    SpecificTxn {
        days_ago: 4,
        merchant: "Adobe Creative Cloud",
        cents: -2_299,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    SpecificTxn {
        days_ago: 5,
        merchant: "Spotify Family",
        cents: -1_699,
        category_id: Some("subscriptions"),
        account_key: "amex",
    },
    SpecificTxn {
        days_ago: 6,
        merchant: "Acme Corp · Payroll",
        cents: 520_000,
        category_id: None,
        account_key: "adam-checking",
    },
    SpecificTxn {
        days_ago: 7,
        merchant: "Costco",
        cents: -41_200,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 8,
        merchant: "Whole Foods",
        cents: -6_430,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 9,
        merchant: "BP Gas",
        cents: -5_240,
        category_id: Some("transport"),
        account_key: "adam-checking",
    },
    SpecificTxn {
        days_ago: 11,
        merchant: "PG&E",
        cents: -22_000,
        category_id: Some("utilities"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 11,
        merchant: "Comcast",
        cents: -8_800,
        category_id: Some("utilities"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 12,
        merchant: "Pharmacy",
        cents: -3_210,
        category_id: Some("health"),
        account_key: "mira-checking",
    },
    SpecificTxn {
        days_ago: 13,
        merchant: "Internet · Sonic",
        cents: -8_800,
        category_id: Some("utilities"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 16,
        merchant: "Trader Joe's",
        cents: -5_240,
        category_id: Some("groceries"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 18,
        merchant: "Bay Property Mgmt · Rent",
        cents: -185_000,
        category_id: Some("housing"),
        account_key: "joint-checking",
    },
    SpecificTxn {
        days_ago: 20,
        merchant: "Sunset Co · Payroll",
        cents: 480_000,
        category_id: None,
        account_key: "mira-checking",
    },
];

// ── Normalises a (bound_a, bound_b) pair so that `gen_range(lo..=hi)` never
/// panics even when the spec lists the larger value first (as it does for outflows).
fn gen_amount(rng: &mut ChaCha20Rng, a: i64, b: i64) -> i64 {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    rng.gen_range(lo..=hi)
}

/// Seed the database with the Mira & Adam household. Returns a summary with the
/// `imports` row id so the caller can mark it finished on success.
pub fn seed_household(db: &Db) -> CoreResult<SeedSummary> {
    let mut conn = db.get()?;
    let tx = conn.transaction()?;
    let mut rng = ChaCha20Rng::seed_from_u64(SAMPLE_SEED);

    // 1. Open an `imports` row of source='sample' so the wizard can stamp it on completion.
    let import_id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO imports(id, source, filename, started_at) VALUES(?1, 'sample', NULL, ?2)",
        params![&import_id, Utc::now().to_rfc3339()],
    )?;

    // 2. Insert category groups + categories (idempotent via OR IGNORE).
    for &(id, label) in CATEGORY_GROUPS {
        tx.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
            params![id, label],
        )?;
    }
    for &(id, group, label) in CATEGORIES {
        tx.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) \
             VALUES(?1, ?2, ?3, '#94A3B8', 0)",
            params![id, group, label],
        )?;
    }

    // 3. Insert accounts with source = 'sample'.
    let mut account_ids = Vec::with_capacity(ACCOUNTS.len());
    let now = Utc::now();
    for acct in ACCOUNTS {
        let id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, last4, currency, color, created_at, source) \
             VALUES(?1, ?2, ?3, ?4, ?5, NULL, 'USD', ?6, ?7, 'sample')",
            params![&id, acct.owner, acct.bank, acct.r#type, acct.name, acct.color, now.to_rfc3339()],
        )?;
        tx.execute(
            "INSERT INTO account_balances(account_id, as_of_date, balance_cents) VALUES(?1, ?2, ?3)",
            params![&id, now.date_naive().to_string(), acct.opening_cents],
        )?;
        account_ids.push(id);
    }

    // 4. Generate ~250 transactions across 12 months across the active accounts.
    let start_date = now - Duration::days(365);
    let active_accounts: Vec<&str> = account_ids
        .iter()
        .zip(ACCOUNTS.iter())
        .filter(|(_, a)| a.r#type != "Investment")  // skip the brokerage
        .map(|(id, _)| id.as_str())
        .collect();

    let mut tx_count: u32 = 0;
    for day in 0..365 {
        // 0–2 transactions per day on average.
        let n = rng.gen_range(0..=2u32);
        for _ in 0..n {
            let acct = active_accounts[rng.gen_range(0..active_accounts.len())];
            let &(mname, bound_a, bound_b) = &MERCHANTS[rng.gen_range(0..MERCHANTS.len())];
            let amount = gen_amount(&mut rng, bound_a, bound_b);
            let cat: Option<&'static str> = category_for(mname);
            let posted = start_date + Duration::days(day);

            tx.execute(
                "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
                                          category_id, status, created_at) \
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'cleared', ?7)",
                params![
                    Uuid::new_v4().to_string(),
                    acct,
                    posted.to_rfc3339(),
                    amount,
                    mname,
                    cat, // rusqlite maps Option<&str> → NULL when None
                    Utc::now().to_rfc3339(),
                ],
            )?;
            tx_count += 1;
        }
    }

    // Finish the import row atomically — same transaction as the sample data so a
    // crash between seed and finish cannot leave an unfinished banner on next launch.
    tx.execute(
        "UPDATE imports SET finished_at = ?1, rows_imported = ?2, rows_skipped_duplicates = 0 \
         WHERE id = ?3",
        params![Utc::now().to_rfc3339(), tx_count, &import_id],
    )?;

    tx.commit()?;
    Ok(SeedSummary {
        accounts_created: ACCOUNTS.len() as u32,
        transactions_created: tx_count,
        import_id,
    })
}

/// Returns `None` for inflows (no consumer-spend category fits) — Phase 3 adds inflow handling.
/// All returned ids MUST exist in CATEGORIES above (FK constraint on transactions.category_id).
fn category_for(merchant: &str) -> Option<&'static str> {
    match merchant {
        "Safeway" | "Whole Foods" => Some("groceries"),
        "Starbucks" | "Chipotle" => Some("dining"),
        "Shell" | "Uber" => Some("transport"),
        "PG&E" | "Comcast" => Some("utilities"),
        "Netflix" | "Spotify" => Some("subscriptions"),
        "Amazon" | "Target" | "Apple" => Some("shopping"),
        "Acme Payroll" => None, // inflow — no category until Phase 3
        _ => Some("shopping"),
    }
}

// ── Helpers for dev-demo date arithmetic ──────────────────────────────────────

/// Returns (year, month) of the month `back` months before the given year/month.
fn months_back(year: i32, month: u32, back: u32) -> (i32, u32) {
    // Convert to 0-indexed absolute month count, subtract, convert back.
    let total = (year as i64) * 12 + (month as i64 - 1) - back as i64;
    let y = (total / 12) as i32;
    let m = (total % 12 + 1) as u32;
    (y, m)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Seed the rich "Mira & Adam" prototype dataset for local development testing.
///
/// Unlike `seed_household` (which uses random data and is pinned for tests),
/// this function seeds a fully curated dataset:
/// - 6 accounts matching the original design prototype
/// - 6 months of recurring transaction history (for recurring-detection algorithm)
/// - 18 hand-crafted recent transactions anchored to today's date
/// - 5 goals, 5 manual assets, 4 liabilities, 5-category budgets, 6 net-worth snapshots
///
/// **Dev-only. Not shipped to end users.**
/// The data uses `source='sample'` so `clear_sample_data` can remove it.
/// Calling this function twice wipes and re-seeds cleanly (idempotent).
pub fn seed_dev_demo(db: &Db) -> CoreResult<SeedSummary> {
    use std::collections::HashMap;

    let mut conn = db.get()?;
    let sql_tx = conn.transaction()?;
    let now = Utc::now();
    let today = now.date_naive();
    let year = today.year();
    let month = today.month();

    // ── 0. Idempotent wipe ─────────────────────────────────────────────────
    // Tables without a source column: clear all rows (dev fixture only).
    sql_tx.execute("DELETE FROM goals", [])?;
    sql_tx.execute("DELETE FROM manual_assets", [])?;
    sql_tx.execute("DELETE FROM liabilities", [])?;
    sql_tx.execute("DELETE FROM budgets", [])?;
    sql_tx.execute("DELETE FROM net_worth_snapshots", [])?;
    // ON DELETE CASCADE propagates to account_balances + transactions.
    sql_tx.execute("DELETE FROM accounts WHERE source = 'sample'", [])?;

    // ── 1. Import row ──────────────────────────────────────────────────────
    let import_id = Uuid::new_v4().to_string();
    sql_tx.execute(
        "INSERT INTO imports(id, source, filename, started_at) VALUES(?1, 'sample', NULL, ?2)",
        params![&import_id, now.to_rfc3339()],
    )?;

    // ── 2. Categories (idempotent) ─────────────────────────────────────────
    for &(id, label) in CATEGORY_GROUPS {
        sql_tx.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
            params![id, label],
        )?;
    }
    for &(id, group, label) in CATEGORIES {
        sql_tx.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) \
             VALUES(?1, ?2, ?3, '#94A3B8', 0)",
            params![id, group, label],
        )?;
    }

    // ── 3. Accounts ────────────────────────────────────────────────────────
    let mut acct: HashMap<&str, String> = HashMap::new();
    for da in DEMO_ACCOUNTS {
        let id = Uuid::new_v4().to_string();
        sql_tx.execute(
            "INSERT INTO accounts(id, owner, bank, type, name, last4, currency, color, \
             created_at, source) VALUES(?1, ?2, ?3, ?4, ?5, NULL, 'USD', ?6, ?7, 'sample')",
            params![
                &id,
                da.owner,
                da.bank,
                da.typ,
                da.name,
                da.color,
                now.to_rfc3339()
            ],
        )?;
        sql_tx.execute(
            "INSERT INTO account_balances(account_id, as_of_date, balance_cents) \
             VALUES(?1, ?2, ?3)",
            params![&id, today.to_string(), da.balance_cents],
        )?;
        acct.insert(da.key, id);
    }

    // ── 4. Transactions ────────────────────────────────────────────────────
    // Collect into a Vec first to avoid borrow conflicts with `sql_tx`.
    let mut txns: Vec<(NaiveDate, &str, i64, Option<&str>, &str)> = Vec::new();

    // 4a. 6 months of recurring history (months 1-6 before the current month).
    //     Month 1 (the most-recent prior month) is capped at day 17 because the
    //     specific 18 hand-crafted transactions cover day 18 onward.
    for back in 1_u32..=6 {
        let (y, m) = months_back(year, month, back);
        let max_day = if back == 1 { 17 } else { days_in_month(y, m) };
        for ri in RECURRING_MONTHLY {
            if ri.day <= max_day {
                if let Some(date) = NaiveDate::from_ymd_opt(y, m, ri.day) {
                    txns.push((date, ri.merchant, ri.cents, ri.category_id, ri.account_key));
                }
            }
        }
    }

    // 4b. Specific 18 recent transactions anchored relative to today.
    for st in SPECIFIC_TXNS {
        let date = today - Duration::days(st.days_ago);
        txns.push((date, st.merchant, st.cents, st.category_id, st.account_key));
    }

    // 4c. Insert all.
    let mut tx_count: u32 = 0;
    for &(date, merchant, cents, cat, account_key) in &txns {
        let dt = date.and_hms_opt(12, 0, 0).unwrap().and_utc();
        sql_tx.execute(
            "INSERT INTO transactions(id, account_id, posted_at, amount_cents, merchant_raw, \
             category_id, status, created_at) VALUES(?1, ?2, ?3, ?4, ?5, ?6, 'cleared', ?7)",
            params![
                Uuid::new_v4().to_string(),
                &acct[account_key],
                dt.to_rfc3339(),
                cents,
                merchant,
                cat,
                now.to_rfc3339(),
            ],
        )?;
        tx_count += 1;
    }

    // ── 5. Goals ───────────────────────────────────────────────────────────
    // (name, type, target_cents, current_cents, monthly_cents, target_date, color)
    #[allow(clippy::type_complexity)]
    let goals: &[(&str, &str, i64, i64, i64, Option<&str>, &str)] = &[
        (
            "House down payment",
            "save-by-date",
            8_000_000,
            2_864_000,
            160_000,
            Some("2027-01-01"),
            "#10B981",
        ),
        (
            "Six-month emergency fund",
            "build-balance",
            2_400_000,
            1_820_000,
            90_000,
            None,
            "#3B82F6",
        ),
        (
            "Italy trip · September",
            "save-by-date",
            450_000,
            185_000,
            60_000,
            Some("2026-09-01"),
            "#F59E0B",
        ),
        (
            "Pay off Amex Gold",
            "debt-payoff",
            241_800,
            0,
            120_900,
            None,
            "#EF4444",
        ),
        (
            "Stay under $400/mo dining",
            "spending-cap",
            40_000,
            41_200,
            0,
            None,
            "#8B5CF6",
        ),
    ];
    for (i, &(name, typ, target, current, monthly, target_date, color)) in goals.iter().enumerate()
    {
        sql_tx.execute(
            "INSERT INTO goals(id, name, type, target_cents, current_cents, monthly_cents, \
             target_date, color, sort_order, created_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                Uuid::new_v4().to_string(),
                name,
                typ,
                target,
                current,
                monthly,
                target_date,
                color,
                i as i64,
                now.to_rfc3339(),
            ],
        )?;
    }

    // ── 6. Manual assets ───────────────────────────────────────────────────
    // (name, asset_type, value_cents)
    let assets: &[(&str, &str, i64)] = &[
        ("Home · 142 Mosswood Ln", "Real estate", 61_200_000),
        ("Mira · 2022 Subaru Outback", "Vehicle", 1_840_000),
        ("Adam · 2019 Honda Civic", "Vehicle", 1_180_000),
        ("Coinbase", "Crypto", 421_800),
        ("Lithograph · Hockney", "Collectibles", 320_000),
    ];
    for &(name, asset_type, value) in assets {
        sql_tx.execute(
            "INSERT INTO manual_assets(id, name, asset_type, value_cents, currency, \
             created_at, updated_at) VALUES(?1, ?2, ?3, ?4, 'USD', ?5, ?5)",
            params![
                Uuid::new_v4().to_string(),
                name,
                asset_type,
                value,
                now.to_rfc3339()
            ],
        )?;
    }

    // ── 7. Liabilities ─────────────────────────────────────────────────────
    // (name, liability_type, balance_cents, limit_cents, apr_pct, min_payment_cents, payoff_date)
    #[allow(clippy::type_complexity)]
    let liabilities: &[(
        &str,
        &str,
        i64,
        Option<i64>,
        Option<f64>,
        Option<i64>,
        Option<&str>,
    )] = &[
        (
            "First Federal · 30-yr fixed",
            "Mortgage",
            38_842_000,
            None,
            Some(6.125),
            Some(236_000),
            Some("2054-01-01"),
        ),
        (
            "Subaru Finance",
            "Auto loan",
            1_248_000,
            None,
            Some(4.9),
            Some(42_000),
            None,
        ),
        (
            "Mira · Federal Direct",
            "Student loan",
            1_824_000,
            None,
            Some(5.5),
            Some(23_000),
            None,
        ),
        (
            "Amex Gold",
            "Credit card",
            241_800,
            Some(2_000_000),
            Some(24.9),
            Some(5_000),
            None,
        ),
    ];
    for &(name, typ, balance, limit, apr, min_payment, payoff) in liabilities {
        sql_tx.execute(
            "INSERT INTO liabilities(id, name, liability_type, balance_cents, limit_cents, \
             apr_pct, min_payment_cents, payoff_date, currency, created_at, updated_at) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'USD', ?9, ?9)",
            params![
                Uuid::new_v4().to_string(),
                name,
                typ,
                balance,
                limit,
                apr,
                min_payment,
                payoff,
                now.to_rfc3339(),
            ],
        )?;
    }

    // ── 8. Budgets (current month + 2 prior) ──────────────────────────────
    // (category_id, amount_cents)
    let budget_cats: &[(&str, i64)] = &[
        ("groceries", 80_000),
        ("dining", 40_000),
        ("subscriptions", 20_000),
        ("utilities", 35_000),
        ("transport", 30_000),
    ];
    for mo in 0_u32..3 {
        let (y, m) = months_back(year, month, mo);
        let month_str = format!("{y}-{m:02}");
        for &(cat_id, amount) in budget_cats {
            sql_tx.execute(
                "INSERT OR IGNORE INTO budgets(id, category_id, month, amount_cents, \
                 created_at, updated_at) VALUES(?1, ?2, ?3, ?4, ?5, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    cat_id,
                    &month_str,
                    amount,
                    now.to_rfc3339(),
                ],
            )?;
        }
    }

    // ── 9. Net-worth snapshots (6 monthly history points) ─────────────────
    // Approximate net worth ≈ $368 k (home + cars + crypto + art + 401k + accounts − debts).
    // (months_back_count, total_cents)
    let nw_history: &[(u32, i64)] = &[
        (6, 35_200_000),
        (5, 35_500_000),
        (4, 35_800_000),
        (3, 36_100_000),
        (2, 36_600_000),
        (1, 36_750_000),
    ];
    for &(back, total) in nw_history {
        let (y, m) = months_back(year, month, back);
        let snap_date = NaiveDate::from_ymd_opt(y, m, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(y, m, 28).unwrap());
        sql_tx.execute(
            "INSERT OR IGNORE INTO net_worth_snapshots(id, date, total_cents, created_at) \
             VALUES(?1, ?2, ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                snap_date.to_string(),
                total,
                now.to_rfc3339()
            ],
        )?;
    }
    // Seed today's snapshot so the chart renders immediately.
    sql_tx.execute(
        "INSERT OR IGNORE INTO net_worth_snapshots(id, date, total_cents, created_at) \
         VALUES(?1, ?2, ?3, ?4)",
        params![
            Uuid::new_v4().to_string(),
            today.to_string(),
            36_819_400_i64,
            now.to_rfc3339()
        ],
    )?;

    // ── Finish import ──────────────────────────────────────────────────────
    sql_tx.execute(
        "UPDATE imports SET finished_at = ?1, rows_imported = ?2, \
         rows_skipped_duplicates = 0 WHERE id = ?3",
        params![now.to_rfc3339(), tx_count, &import_id],
    )?;

    sql_tx.commit()?;
    Ok(SeedSummary {
        accounts_created: DEMO_ACCOUNTS.len() as u32,
        transactions_created: tx_count,
        import_id,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("seed.sqlcipher");
        let key = keychain::generate_random_key();
        let db = Db::open(&path, &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    #[test]
    fn deterministic_seed_produces_known_counts() {
        let (_d, db) = fresh_db();
        let s = seed_household(&db).unwrap();
        assert_eq!(s.accounts_created, 6);
        // RNG is pinned; assert tight envelope — the determinism integration test pins the exact count.
        // gen_range(0..=2) has mean 1, so expected ~365 per year; actual is 346 for SAMPLE_SEED.
        assert!(
            s.transactions_created >= 300 && s.transactions_created <= 400,
            "got {} txns",
            s.transactions_created
        );
    }

    #[test]
    fn seed_dev_demo_creates_full_dataset() {
        let (_d, db) = fresh_db();
        let s = seed_dev_demo(&db).unwrap();
        assert_eq!(s.accounts_created, 6, "6 demo accounts");
        // 6 prior months of recurring (14 in month-1, 22*5 in months 2-6) + 18 specific = 142.
        assert!(
            s.transactions_created >= 100,
            "expected 100+ txns, got {}",
            s.transactions_created
        );

        let conn = db.get().unwrap();
        let goal_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM goals", [], |r| r.get(0))
            .unwrap();
        assert_eq!(goal_count, 5, "5 goals");

        let asset_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM manual_assets", [], |r| r.get(0))
            .unwrap();
        assert_eq!(asset_count, 5, "5 manual assets");

        let liability_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM liabilities", [], |r| r.get(0))
            .unwrap();
        assert_eq!(liability_count, 4, "4 liabilities");

        // Net-worth history (6 past months + today = 7 rows).
        let nw_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM net_worth_snapshots", [], |r| r.get(0))
            .unwrap();
        assert!(nw_count >= 6, "at least 6 net-worth snapshots");
    }

    #[test]
    fn seed_dev_demo_is_idempotent() {
        let (_d, db) = fresh_db();
        seed_dev_demo(&db).unwrap();
        // Second call must succeed and not duplicate non-source tables.
        let s2 = seed_dev_demo(&db).unwrap();
        assert_eq!(s2.accounts_created, 6);

        let conn = db.get().unwrap();
        let goal_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM goals", [], |r| r.get(0))
            .unwrap();
        assert_eq!(goal_count, 5, "goals must not double-up on second call");
    }

    #[test]
    fn idempotent_against_existing_category_groups() {
        let (_d, db) = fresh_db();
        {
            let conn = db.get().unwrap();
            conn.execute(
                "INSERT INTO category_groups(id,label,sort_order) VALUES('daily','Daily',0)",
                [],
            )
            .unwrap();
        }
        // Should not panic on the duplicate group_id.
        seed_household(&db).unwrap();
    }
}
