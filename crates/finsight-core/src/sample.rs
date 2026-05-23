//! Procedural "Mira & Adam" sample household — used by the onboarding wizard's
//! "Try with sample data" path. Seeded with a pinned constant so tests can assert
//! exact row counts and a known first transaction.
//! NOTE: Mira & Adam are fictional characters used only as sample data labels.

use crate::error::CoreResult;
use crate::Db;
use chrono::{Duration, Utc};
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

/// Normalises a (bound_a, bound_b) pair so that `gen_range(lo..=hi)` never
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
