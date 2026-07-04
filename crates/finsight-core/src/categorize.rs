//! Deterministic, provider-free transaction categorization.
//!
//! The AI categorizer (rules → LLM fallback) only runs when a completion
//! provider is configured, and there are no built-in rules — so a fresh import
//! with no provider leaves every transaction "Uncategorized". This module gives
//! a stable baseline: a fixed keyword → category map that assigns a category the
//! same way on every import, with no provider and no network.
//!
//! Design rules:
//! - Only map merchants we are confident about. Ambiguous names are left
//!   uncategorized rather than forced into a wrong bucket.
//! - Only assign categories that actually exist in the user's `categories`
//!   table. If the user deleted a starter category, matching transactions stay
//!   uncategorized instead of resurrecting it.
//! - Matches are recorded with `source = "builtin"` and confidence 1.0, so the
//!   LLM pass (which loads `category_id IS NULL`) skips them and they do not
//!   land in the low-confidence review queue.

use crate::error::CoreResult;
use rusqlite::{params, Connection};
use std::collections::HashSet;
use uuid::Uuid;

/// Ordered keyword table. Each entry maps a lowercase substring found in
/// `merchant_raw` to a starter category id. Earlier entries win, so put more
/// specific patterns (e.g. "uber eats") before broader ones (e.g. "uber").
const KEYWORD_MAP: &[(&str, &str)] = &[
    // ── Dining ────────────────────────────────────────────────────────────
    ("uber eats", "dining"),
    ("ubereats", "dining"),
    ("doordash", "dining"),
    ("skipthedishes", "dining"),
    ("grubhub", "dining"),
    ("hungerhub", "dining"),
    ("swiggy", "dining"),
    ("starbucks", "dining"),
    ("tim hortons", "dining"),
    ("mcdonald", "dining"),
    ("dominos", "dining"),
    ("domino's", "dining"),
    ("pizza", "dining"),
    ("chipotle", "dining"),
    ("freshslice", "dining"),
    ("subway", "dining"),
    ("burger", "dining"),
    ("sushi", "dining"),
    ("ramen", "dining"),
    ("bakery", "dining"),
    ("cafe", "dining"),
    ("café", "dining"),
    ("coffee", "dining"),
    ("donut", "dining"),
    ("restaurant", "dining"),
    ("sweetgreen", "dining"),
    ("blue bottle", "dining"),
    ("the keg", "dining"),
    ("a&w", "dining"),
    (" kfc", "dining"),
    ("popeyes", "dining"),
    ("wendy", "dining"),
    ("tacofino", "dining"),
    ("olive garden", "dining"),
    ("breka", "dining"),
    ("cilantro", "dining"),
    ("madras", "dining"),
    // ── Groceries ─────────────────────────────────────────────────────────
    ("walmart", "groceries"),
    ("wal-mart", "groceries"),
    ("save on foods", "groceries"),
    ("save-on-foods", "groceries"),
    ("saveonfoods", "groceries"),
    ("safeway", "groceries"),
    ("costco", "groceries"),
    ("superstore", "groceries"),
    ("no frills", "groceries"),
    ("loblaws", "groceries"),
    ("sobey", "groceries"),
    ("whole foods", "groceries"),
    ("trader joe", "groceries"),
    ("fairway market", "groceries"),
    ("freshco", "groceries"),
    ("food basics", "groceries"),
    ("t&t supermarket", "groceries"),
    ("instacart", "groceries"),
    // ── Transport ─────────────────────────────────────────────────────────
    ("uber trip", "transport"),
    ("uber holdings", "transport"),
    ("uber canada", "transport"),
    ("uber *", "transport"),
    ("uber\t", "transport"),
    ("lyft", "transport"),
    ("evo car share", "transport"),
    ("compass", "transport"), // Vancouver transit Compass card
    ("arc transit", "transport"),
    ("translink", "transport"),
    ("transit", "transport"),
    ("parking", "transport"),
    ("chevron", "transport"),
    ("imperial chev", "transport"),
    ("petro-canada", "transport"),
    ("petrocan", "transport"),
    ("esso", "transport"),
    (" shell", "transport"),
    ("bp gas", "transport"),
    ("husky", "transport"),
    // ── Subscriptions ─────────────────────────────────────────────────────
    ("spotify", "subscriptions"),
    ("netflix", "subscriptions"),
    ("disney+", "subscriptions"),
    ("disney plus", "subscriptions"),
    ("openai", "subscriptions"),
    ("chatgpt", "subscriptions"),
    ("anthropic", "subscriptions"),
    ("claude.ai", "subscriptions"),
    ("openrouter", "subscriptions"),
    ("adobe", "subscriptions"),
    ("icloud", "subscriptions"),
    ("notion", "subscriptions"),
    ("audible", "subscriptions"),
    ("patreon", "subscriptions"),
    ("nytimes", "subscriptions"),
    ("youtube premium", "subscriptions"),
    ("amazon prime", "subscriptions"),
    ("membership fee", "subscriptions"),
    ("dropbox", "subscriptions"),
    // ── Utilities (incl. phone/internet) ──────────────────────────────────
    ("virgin plus", "utilities"),
    ("freedom mobile", "utilities"),
    ("telus", "utilities"),
    ("rogers", "utilities"),
    ("shaw", "utilities"),
    ("bell canada", "utilities"),
    ("bell mobility", "utilities"),
    ("fido", "utilities"),
    ("koodo", "utilities"),
    ("bc hydro", "utilities"),
    ("fortis", "utilities"),
    ("epcor", "utilities"),
    ("enmax", "utilities"),
    ("hydro", "utilities"),
    ("comcast", "utilities"),
    ("pg&e", "utilities"),
    // ── Travel ────────────────────────────────────────────────────────────
    ("aircanada", "travel"),
    ("air canada", "travel"),
    ("westjet", "travel"),
    ("united airlines", "travel"),
    ("delta air", "travel"),
    ("flighthub", "travel"),
    ("expedia", "travel"),
    ("airbnb", "travel"),
    ("booking.com", "travel"),
    ("wanderu", "travel"),
    ("auberge", "travel"),
    ("marriott", "travel"),
    ("hilton", "travel"),
    ("porter airlines", "travel"),
    // ── Shopping ──────────────────────────────────────────────────────────
    ("amazon", "shopping"),
    ("temu", "shopping"),
    ("dollarama", "shopping"),
    ("best buy", "shopping"),
    ("bestbuy", "shopping"),
    ("marshalls", "shopping"),
    ("winners", "shopping"),
    ("h&m", "shopping"),
    ("hennes", "shopping"),
    ("urban planet", "shopping"),
    ("staples", "shopping"),
    ("home depot", "shopping"),
    ("ikea", "shopping"),
    ("canadian tire", "shopping"),
    ("hudson's bay", "shopping"),
    ("aliexpress", "shopping"),
    // ── Health ────────────────────────────────────────────────────────────
    ("shoppers drug mart", "health"),
    ("pharmacy", "health"),
    ("pharmaprix", "health"),
    ("rexall", "health"),
    ("dental", "health"),
    ("clinic", "health"),
    ("physio", "health"),
    ("progressivehealt", "health"),
    // ── Housing ───────────────────────────────────────────────────────────
    ("property mgmt", "housing"),
    ("property management", "housing"),
    ("mortgage", "housing"),
    (" rent ", "housing"),
];

/// Best-effort deterministic category for a merchant string. Returns the
/// starter-category id, or `None` if nothing confidently matches.
pub fn builtin_category(merchant_raw: &str) -> Option<&'static str> {
    let m = merchant_raw.to_lowercase();
    for (needle, cat) in KEYWORD_MAP {
        if m.contains(needle) {
            return Some(cat);
        }
    }
    None
}

/// Merchant substrings that identify a transfer between the user's own accounts
/// (or a credit-card payment) rather than real income or spending.
const TRANSFER_KEYWORDS: &[&str] = &[
    "payment received - thank you", // credit-card payment
    "payment - thank you",
    "autopay",
    "internet deposit from",   // Tangerine internal transfer in
    "internet withdrawal to",  // Tangerine internal transfer out
    "e-transfer",              // INTERAC e-Transfer to/from self or a friend
    "e transfer",
    "email money transfer",
    "transfer to",
    "transfer from",
    "internal transfer",
    "online banking transfer",
    "tfr-to",
    "tfr-from",
];

/// True when a merchant string looks like an internal money movement (transfer
/// or credit-card payment) that must not count as income or spending.
pub fn is_transfer(merchant_raw: &str) -> bool {
    let m = merchant_raw.to_lowercase();
    TRANSFER_KEYWORDS.iter().any(|kw| m.contains(kw))
}

/// Keywords that identify the *credit-card side* of a card payment (the leg
/// posted to the card account). Used by pairing rule B: only this specific
/// pattern is trusted enough to pull in an unflagged bank-side leg.
const CC_PAYMENT_KEYWORDS: &[&str] = &[
    "payment received - thank you",
    "payment - thank you",
    "autopay",
];

fn is_cc_payment(merchant_raw: &str) -> bool {
    let m = merchant_raw.to_lowercase();
    CC_PAYMENT_KEYWORDS.iter().any(|kw| m.contains(kw))
}

/// Merchant hints that make an *unflagged* bank-account leg eligible to pair
/// with a credit-card payment leg (rule B). Deliberately narrow: generic words
/// like "preauthorized payment" alone must NOT qualify — a gym membership debit
/// is not a card payment. The hint must reference bill-payment mechanics or a
/// card network / issuer.
const CC_COUNTERPARTY_HINTS: &[&str] = &[
    "bill payment",
    "bill pay",
    "billpay",
    "credit card",
    "card payment",
    "amex",
    "american express",
    "visa",
    "mastercard",
    "master card",
    "capital one",
    "mbna",
];

fn has_cc_counterparty_hint(merchant_raw: &str) -> bool {
    let m = merchant_raw.to_lowercase();
    CC_COUNTERPARTY_HINTS.iter().any(|kw| m.contains(kw))
}

/// Maximum calendar-day gap between the two legs of one transfer. Bill payments
/// commonly post 1–3 business days apart; 4 covers a weekend in between.
const PAIR_WINDOW_DAYS: i64 = 4;

#[derive(Debug, Clone)]
struct PairCandidate {
    id: String,
    account_id: String,
    account_type: String,
    day: i64, // julian day number of posted_at's date
    amount_cents: i64,
    merchant_raw: String,
    flagged: bool,
}

/// Pair the two legs of a cross-account transfer: a withdrawal in one account
/// matched to the equal-and-opposite deposit in another, within a small date
/// window. Writes reciprocal `transfer_peer_id` on both rows and sets
/// `is_transfer = 1` on both (rule B can flag a leg keywords missed).
///
/// Matching is deliberately conservative:
/// - Rule A: both legs already flagged `is_transfer` (keyword pass) — e.g.
///   "Internet Withdrawal to …" ↔ "Internet Deposit from …".
/// - Rule B: the flagged leg is a credit-card payment ("PAYMENT RECEIVED -
///   THANK YOU") posted to a Credit account, and the other leg's merchant
///   carries a bill-payment / card-network hint. Only then is an unflagged
///   leg pulled in.
/// Amounts must be exactly opposite, accounts must differ, the gap must be
/// ≤ 4 days, and each transaction pairs at most once (nearest date wins).
///
/// Idempotent: already-paired rows are never touched, so a re-run after new
/// imports only pairs new legs. Returns the number of pairs created.
pub fn pair_transfers(conn: &mut Connection) -> CoreResult<u32> {
    let mut candidates: Vec<PairCandidate> = {
        let mut stmt = conn.prepare(
            "SELECT t.id, t.account_id, a.type, CAST(julianday(date(t.posted_at)) AS INTEGER), \
                    t.amount_cents, t.merchant_raw, t.is_transfer \
             FROM transactions t JOIN accounts a ON a.id = t.account_id \
             WHERE t.transfer_peer_id IS NULL AND t.amount_cents != 0 \
             ORDER BY t.posted_at, t.id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(PairCandidate {
                id: r.get(0)?,
                account_id: r.get(1)?,
                account_type: r.get(2)?,
                day: r.get(3)?,
                amount_cents: r.get(4)?,
                merchant_raw: r.get(5)?,
                flagged: r.get::<_, i64>(6)? != 0,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };
    // Keep only rows that could participate at all: flagged legs, or unflagged
    // legs carrying a card-payment counterparty hint (rule B's bank side).
    candidates.retain(|c| c.flagged || has_cc_counterparty_hint(&c.merchant_raw));

    let eligible = |a: &PairCandidate, b: &PairCandidate| -> bool {
        if a.account_id == b.account_id {
            return false;
        }
        if a.amount_cents != -b.amount_cents {
            return false;
        }
        if (a.day - b.day).abs() > PAIR_WINDOW_DAYS {
            return false;
        }
        if a.flagged && b.flagged {
            return true; // rule A
        }
        // Rule B: exactly one leg flagged, and it must be a credit-card payment
        // on a Credit account; the unflagged leg must carry a counterparty hint.
        let (flagged, other) = match (a.flagged, b.flagged) {
            (true, false) => (a, b),
            (false, true) => (b, a),
            _ => return false,
        };
        flagged.account_type == "Credit"
            && is_cc_payment(&flagged.merchant_raw)
            && has_cc_counterparty_hint(&other.merchant_raw)
    };

    // Greedy nearest-date matching. Candidates are in (posted_at, id) order, so
    // iteration and tie-breaks are deterministic.
    let mut used: HashSet<usize> = HashSet::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for i in 0..candidates.len() {
        if used.contains(&i) || !candidates[i].flagged {
            continue; // every pair has at least one flagged leg — anchor on it
        }
        let mut best: Option<usize> = None;
        for j in 0..candidates.len() {
            if i == j || used.contains(&j) {
                continue;
            }
            if !eligible(&candidates[i], &candidates[j]) {
                continue;
            }
            let better = match best {
                None => true,
                Some(b) => {
                    (candidates[i].day - candidates[j].day).abs()
                        < (candidates[i].day - candidates[b].day).abs()
                }
            };
            if better {
                best = Some(j);
            }
        }
        if let Some(j) = best {
            used.insert(i);
            used.insert(j);
            pairs.push((candidates[i].id.clone(), candidates[j].id.clone()));
        }
    }

    let tx = conn.transaction()?;
    for (a, b) in &pairs {
        tx.execute(
            "UPDATE transactions SET transfer_peer_id = ?1, is_transfer = 1 WHERE id = ?2",
            params![b, a],
        )?;
        tx.execute(
            "UPDATE transactions SET transfer_peer_id = ?1, is_transfer = 1 WHERE id = ?2",
            params![a, b],
        )?;
    }
    tx.commit()?;
    Ok(pairs.len() as u32)
}

/// The standard starter categories the built-in categorizer targets. Grouped
/// so they slot into the conscious-spending breakdown. `(id, group_id, label)`.
const DEFAULT_CATEGORIES: &[(&str, &str, &str)] = &[
    ("dining", "daily", "Dining"),
    ("groceries", "daily", "Groceries"),
    ("transport", "daily", "Transport"),
    ("shopping", "lifestyle", "Shopping"),
    ("travel", "lifestyle", "Travel"),
    ("gifts", "lifestyle", "Gifts"),
    ("housing", "fixed", "Housing"),
    ("utilities", "fixed", "Utilities"),
    ("subscriptions", "fixed", "Subscriptions"),
    ("health", "wellbeing", "Health"),
];
const DEFAULT_GROUPS: &[(&str, &str)] = &[
    ("fixed", "Fixed"),
    ("daily", "Daily"),
    ("lifestyle", "Lifestyle"),
    ("wellbeing", "Wellbeing"),
];

/// Seed the standard starter categories, but ONLY when the categories table is
/// empty — so a user who imports before completing onboarding's category step
/// still gets their transactions categorized, without ever overwriting a
/// user-configured set. Idempotent and safe to call on every import.
pub fn ensure_default_categories(conn: &mut Connection) -> CoreResult<()> {
    let existing: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0))?;
    if existing > 0 {
        return Ok(());
    }
    let tx = conn.transaction()?;
    for (gid, label) in DEFAULT_GROUPS {
        tx.execute(
            "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES(?1, ?2, 0)",
            params![gid, label],
        )?;
    }
    for (i, (id, group_id, label)) in DEFAULT_CATEGORIES.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, sort_order) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![id, group_id, label, crate::palette::color_for(id), i as i64],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Apply the built-in keyword categorizer to every currently-uncategorized
/// transaction. Only assigns categories that exist in the `categories` table.
/// Returns the number of transactions categorized. Idempotent: a second run
/// touches nothing, because matched rows are no longer `category_id IS NULL`.
pub fn apply_builtin_categorization(conn: &mut Connection) -> CoreResult<u32> {
    // Ensure the categorizer has categories to assign, even when the user
    // imported before completing onboarding's category step.
    ensure_default_categories(conn)?;
    let existing: HashSet<String> = {
        let mut stmt = conn.prepare("SELECT id FROM categories WHERE archived_at IS NULL")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut set = HashSet::new();
        for r in rows {
            set.insert(r?);
        }
        set
    };
    // Note: we do NOT early-return when no categories exist — transfer flagging
    // must still run so report totals stay correct even before the user has set
    // up any categories.

    // Consider every uncategorized transaction (for category + transfer
    // evaluation) plus every currently transfer-flagged one (so a re-run after
    // the keyword list changes can *un-flag* a stale transfer even if it also
    // carries a category, e.g. an "Interac - Purchase" once tagged a transfer).
    let pending: Vec<(String, String, bool, bool)> = {
        let mut stmt = conn.prepare(
            "SELECT id, merchant_raw, category_id IS NULL, transfer_peer_id IS NOT NULL \
             FROM transactions \
             WHERE category_id IS NULL OR is_transfer = 1",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)? != 0,
                r.get::<_, i64>(3)? != 0,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };

    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    let mut count: u32 = 0;
    for (txn_id, merchant, uncategorized, paired) in pending {
        // Transfer detection runs regardless of category state, and is written
        // in BOTH directions so a re-run after the keyword list changes corrects
        // stale flags (e.g. an "Interac - Purchase" no longer treated as a
        // transfer once the over-broad 'interac' keyword was removed).
        // EXCEPT: a leg paired to a peer transaction (`pair_transfers`) is a
        // transfer by construction — rule B pairs legs whose merchants carry no
        // transfer keyword, and un-flagging them here would undo the pairing.
        if !paired {
            tx.execute(
                "UPDATE transactions SET is_transfer = ?1 WHERE id = ?2",
                params![is_transfer(&merchant) as i64, txn_id],
            )?;
        }
        if !uncategorized {
            continue;
        }
        // Invariant: transfers are never categorized (see docs + memory). In
        // practice transfer keywords and the category keyword map are disjoint,
        // but make it structural for paired legs, whose merchants CAN look like
        // ordinary bill payments.
        if paired || is_transfer(&merchant) {
            continue;
        }
        let Some(cat) = builtin_category(&merchant) else {
            continue;
        };
        if !existing.contains(cat) {
            continue;
        }
        tx.execute(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
            params![cat, txn_id],
        )?;
        tx.execute(
            "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) \
             VALUES(?1, ?2, ?3, 'builtin', 1.0, NULL, ?4)",
            params![Uuid::new_v4().to_string(), txn_id, cat, now],
        )?;
        count += 1;
    }
    tx.commit()?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("cz.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_categories(conn: &Connection) {
        conn.execute(
            "INSERT INTO category_groups(id,label,sort_order) VALUES('daily','Daily',0)",
            [],
        )
        .unwrap();
        for (id, label) in [
            ("dining", "Dining"),
            ("groceries", "Groceries"),
            ("transport", "Transport"),
            ("subscriptions", "Subscriptions"),
            ("shopping", "Shopping"),
        ] {
            conn.execute(
                "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES(?1,'daily',?2,'#fff',0)",
                params![id, label],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Credit','Card','USD','#000','manual','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
    }

    fn insert_txn(conn: &Connection, id: &str, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES(?1,'a1','2024-01-01T00:00:00Z',1500,?2,'cleared',0,'2024-01-01T00:00:00Z')",
            params![id, merchant],
        )
        .unwrap();
    }

    #[test]
    fn maps_common_real_sample_merchants() {
        assert_eq!(builtin_category("TIM HORTONS #3356       BURNABY"), Some("dining"));
        assert_eq!(builtin_category("UBER EATS               HTTPS://HELP.UB"), Some("dining"));
        assert_eq!(builtin_category("UBER TRIP               HTTPS://HELP.UB"), Some("transport"));
        assert_eq!(builtin_category("EVO CAR SHARE           BURNABY"), Some("transport"));
        assert_eq!(builtin_category("WALMART SUPERCENTER  BURNABY"), Some("groceries"));
        assert_eq!(builtin_category("SPOTIFY                 STOCKHOLM"), Some("subscriptions"));
        assert_eq!(builtin_category("OPENAI *CHATGPT SUBSCR  SAN FRANCISCO"), Some("subscriptions"));
        assert_eq!(builtin_category("TEMU.COM                VICTORIA"), Some("shopping"));
        assert_eq!(builtin_category("COMPASS VENDING BURN    BURNABY"), Some("transport"));
    }

    #[test]
    fn uber_eats_beats_uber_trip_ordering() {
        // "uber eats" must classify as dining, not transport, despite both
        // containing "uber".
        assert_eq!(builtin_category("UBER EATS TORONTO"), Some("dining"));
        assert_eq!(builtin_category("UBER TRIP TORONTO"), Some("transport"));
    }

    #[test]
    fn detects_transfers_and_cc_payments() {
        assert!(is_transfer("PAYMENT RECEIVED - THANK YOU"));
        assert!(is_transfer("Internet Deposit from Tangerine"));
        assert!(is_transfer("Internet Withdrawal to Tangerine"));
        assert!(is_transfer("INTERAC e-Transfer To: Koushik C"));
        assert!(is_transfer("INTERAC e-Transfer From: BRITISH"));
        assert!(is_transfer("Email Money Transfer to Alice")); // friend transfer
        // Real spending must NOT be flagged as a transfer.
        assert!(!is_transfer("TIM HORTONS #3356 BURNABY"));
        assert!(!is_transfer("WALMART SUPERCENTER BURNABY"));
        assert!(!is_transfer("Interest Paid"));
        // "Interac" is Canada's debit network — a debit PURCHASE or a fee is not
        // a transfer (only the e-Transfer product is, caught by 'e-transfer').
        assert!(!is_transfer("Interac - Purchase - COSTCO WHOLESALE W51"));
        assert!(!is_transfer("Interac Network Usage Charge"));
    }

    #[test]
    fn apply_flags_transfers_on_transactions() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        insert_txn(&conn, "t1", "PAYMENT RECEIVED - THANK YOU");
        insert_txn(&conn, "t2", "Internet Withdrawal to Tangerine");
        insert_txn(&conn, "t3", "TIM HORTONS #3356 BURNABY");

        apply_builtin_categorization(&mut conn).unwrap();

        let is_tf = |id: &str| -> i64 {
            conn.query_row(
                "SELECT is_transfer FROM transactions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(is_tf("t1"), 1, "CC payment flagged as transfer");
        assert_eq!(is_tf("t2"), 1, "internal transfer flagged");
        assert_eq!(is_tf("t3"), 0, "real spending not flagged");
    }

    #[test]
    fn re_run_unflags_a_stale_transfer_even_if_it_carries_a_category() {
        // Regression: a row that was flagged is_transfer=1 AND given a category
        // must get un-flagged on a later re-run once its merchant no longer
        // matches a transfer keyword (the "Interac - Purchase" false positive).
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        insert_txn(&conn, "t1", "Interac - Purchase - COSTCO WHOLESALE");
        // Simulate the stale state: flagged AND categorized.
        conn.execute(
            "UPDATE transactions SET is_transfer = 1, category_id = 'groceries' WHERE id = 't1'",
            [],
        )
        .unwrap();

        apply_builtin_categorization(&mut conn).unwrap();

        let (is_tf, cat): (i64, Option<String>) = conn
            .query_row(
                "SELECT is_transfer, category_id FROM transactions WHERE id = 't1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(is_tf, 0, "stale transfer flag must be cleared");
        assert_eq!(cat.as_deref(), Some("groceries"), "category is preserved");
    }

    #[test]
    fn flags_transfers_and_seeds_categories_on_import_first() {
        // Onboarding imports before categories are committed. Transfer flagging
        // must work, AND the categorizer now auto-seeds the default categories
        // so import-first transactions still get categorized.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Credit','Card','USD','#000','manual','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        insert_txn(&conn, "t1", "PAYMENT RECEIVED - THANK YOU");
        insert_txn(&conn, "t2", "TIM HORTONS #3356 BURNABY");

        let n = apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(n, 1, "TIM HORTONS categorized once default categories auto-seed");
        let is_tf: i64 = conn
            .query_row("SELECT is_transfer FROM transactions WHERE id='t1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(is_tf, 1, "the card payment is still flagged as a transfer");
        let cat2: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='t2'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cat2.as_deref(), Some("dining"));
    }

    #[test]
    fn leaves_ambiguous_merchants_uncategorized() {
        assert_eq!(builtin_category("ANOMALY                 SAN FRANCISCO"), None);
        assert_eq!(builtin_category("FELICITAS               VICTORIA"), None);
        assert_eq!(builtin_category("BBYMARKETPLACE*PULSELAB TORONTO"), None);
        assert_eq!(builtin_category("REVENUE SERVICES BC VIC VICTORIA"), None);
    }

    #[test]
    fn apply_categorizes_only_existing_categories_and_is_idempotent() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        insert_txn(&conn, "t1", "TIM HORTONS #3356 BURNABY"); // dining (exists)
        insert_txn(&conn, "t2", "SPOTIFY STOCKHOLM"); // subscriptions (exists)
        insert_txn(&conn, "t3", "ANOMALY SAN FRANCISCO"); // no match
        insert_txn(&conn, "t4", "AUBERGE QUEBEC"); // travel category NOT seeded

        let n = apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(n, 2, "only t1 and t2 map to existing categories");

        let cat1: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cat1.as_deref(), Some("dining"));
        let cat4: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='t4'", [], |r| r.get(0))
            .unwrap();
        assert!(cat4.is_none(), "travel category absent -> stays uncategorized");

        // Idempotent: a second run categorizes nothing new.
        let n2 = apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(n2, 0);

        // Same merchant -> same category on a fresh transaction (stability).
        insert_txn(&conn, "t5", "TIM HORTONS #9999 VICTORIA");
        apply_builtin_categorization(&mut conn).unwrap();
        let cat5: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='t5'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cat5.as_deref(), Some("dining"));
    }

    #[test]
    fn import_first_seeds_default_categories_and_categorizes() {
        // The Phase 4 finding: importing before onboarding's category step left
        // everything uncategorized because no categories existed. apply_builtin
        // now seeds the standard set when the table is empty.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('a1','Me','Bank','Credit','Card','USD','#000','manual','2024-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        // No categories seeded — simulate import-before-onboarding.
        let cats_before: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap();
        assert_eq!(cats_before, 0);

        insert_txn(&conn, "t1", "TIM HORTONS #3356 BURNABY"); // dining
        insert_txn(&conn, "t2", "SPOTIFY STOCKHOLM"); // subscriptions

        let n = apply_builtin_categorization(&mut conn).unwrap();
        assert!(n >= 2, "import-first should now categorize known merchants, got {n}");

        let cats_after: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap();
        assert!(cats_after >= 10, "default categories should be seeded");
        let cat1: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='t1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cat1.as_deref(), Some("dining"));
    }

    // ── pair_transfers ────────────────────────────────────────────────────

    fn seed_accounts_for_pairing(conn: &Connection) {
        for (id, kind, name) in [
            ("chq", "Checking", "Tangerine Chequing"),
            ("sav", "Savings", "Tangerine Savings"),
            ("cc", "Credit", "Amex Cobalt"),
        ] {
            conn.execute(
                "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
                 VALUES(?1,'Me','Bank',?2,?3,'CAD','#000','manual','2024-01-01T00:00:00Z')",
                params![id, kind, name],
            )
            .unwrap();
        }
    }

    fn insert_txn_full(conn: &Connection, id: &str, account: &str, date: &str, cents: i64, merchant: &str) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at) \
             VALUES(?1,?2,?3,?4,?5,'cleared',0,'2024-01-01T00:00:00Z')",
            params![id, account, format!("{date}T12:00:00Z"), cents, merchant],
        )
        .unwrap();
    }

    fn peer_of(conn: &Connection, id: &str) -> Option<String> {
        conn.query_row(
            "SELECT transfer_peer_id FROM transactions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn transfer_flag(conn: &Connection, id: &str) -> i64 {
        conn.query_row(
            "SELECT is_transfer FROM transactions WHERE id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap()
    }

    #[test]
    fn pairs_internal_transfer_when_both_legs_are_flagged() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_accounts_for_pairing(&conn);
        insert_txn_full(&conn, "out", "chq", "2024-03-01", -50_000, "Internet Withdrawal to Tangerine Savings");
        insert_txn_full(&conn, "in", "sav", "2024-03-01", 50_000, "Internet Deposit from Tangerine Chequing");

        apply_builtin_categorization(&mut conn).unwrap();
        let n = pair_transfers(&mut conn).unwrap();

        assert_eq!(n, 1);
        assert_eq!(peer_of(&conn, "out").as_deref(), Some("in"));
        assert_eq!(peer_of(&conn, "in").as_deref(), Some("out"));
    }

    #[test]
    fn pairs_cc_payment_with_bank_bill_payment_and_flags_the_bank_leg() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_accounts_for_pairing(&conn);
        // Card side: keyword-flagged by the builtin pass. Bank side: NO transfer
        // keyword — only pairing can identify it.
        insert_txn_full(&conn, "card", "cc", "2024-03-04", 113_900, "PAYMENT RECEIVED - THANK YOU");
        insert_txn_full(&conn, "bank", "chq", "2024-03-02", -113_900, "Bill Payment to AMEX BANK OF CANADA");

        apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(transfer_flag(&conn, "bank"), 0, "precondition: bank leg not keyword-flagged");

        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 1);
        assert_eq!(peer_of(&conn, "card").as_deref(), Some("bank"));
        assert_eq!(peer_of(&conn, "bank").as_deref(), Some("card"));
        assert_eq!(transfer_flag(&conn, "bank"), 1, "pairing flags the bank leg");

        // A later builtin re-run must NOT un-flag the paired bank leg.
        apply_builtin_categorization(&mut conn).unwrap();
        assert_eq!(transfer_flag(&conn, "bank"), 1, "paired leg survives keyword re-run");
        // And the paired bank leg must never be categorized.
        let cat: Option<String> = conn
            .query_row("SELECT category_id FROM transactions WHERE id='bank'", [], |r| r.get(0))
            .unwrap();
        assert!(cat.is_none(), "paired transfer leg must stay uncategorized");
    }

    #[test]
    fn does_not_pair_same_account_or_beyond_window() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_accounts_for_pairing(&conn);
        // Same account: a reversal, not a transfer pair.
        insert_txn_full(&conn, "s1", "chq", "2024-03-01", -5_000, "Internet Withdrawal to Tangerine Savings");
        insert_txn_full(&conn, "s2", "chq", "2024-03-01", 5_000, "Internet Deposit from Tangerine Savings");
        // Cross-account but 10 days apart: outside the window.
        insert_txn_full(&conn, "w1", "chq", "2024-04-01", -7_000, "Internet Withdrawal to Tangerine Savings");
        insert_txn_full(&conn, "w2", "sav", "2024-04-11", 7_000, "Internet Deposit from Tangerine Chequing");

        apply_builtin_categorization(&mut conn).unwrap();
        let n = pair_transfers(&mut conn).unwrap();

        assert_eq!(n, 0);
        for id in ["s1", "s2", "w1", "w2"] {
            assert!(peer_of(&conn, id).is_none(), "{id} must stay unpaired");
        }
    }

    #[test]
    fn does_not_pair_regular_spending_with_a_flagged_etransfer() {
        // An incoming e-transfer from a friend (+$40, flagged) must not consume
        // an ordinary $40 debit ("PREAUTHORIZED PAYMENT - GYM") as its peer:
        // rule B requires a CC-payment keyword on a Credit account AND a
        // card/bill counterparty hint on the other leg.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_accounts_for_pairing(&conn);
        insert_txn_full(&conn, "etf", "sav", "2024-03-01", 4_000, "INTERAC e-Transfer From: Alice");
        insert_txn_full(&conn, "gym", "chq", "2024-03-01", -4_000, "PREAUTHORIZED PAYMENT - GYM CLUB");

        apply_builtin_categorization(&mut conn).unwrap();
        let n = pair_transfers(&mut conn).unwrap();

        assert_eq!(n, 0);
        assert!(peer_of(&conn, "etf").is_none());
        assert!(peer_of(&conn, "gym").is_none());
        assert_eq!(transfer_flag(&conn, "gym"), 0, "gym debit must not become a transfer");
    }

    #[test]
    fn pairing_is_idempotent_and_prefers_the_nearest_date() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_accounts_for_pairing(&conn);
        // Two same-amount deposits: the one 1 day away must win over 3 days away.
        insert_txn_full(&conn, "out1", "chq", "2024-03-05", -10_000, "Internet Withdrawal to Tangerine Savings");
        insert_txn_full(&conn, "near", "sav", "2024-03-06", 10_000, "Internet Deposit from Tangerine Chequing");
        insert_txn_full(&conn, "far", "sav", "2024-03-08", 10_000, "Internet Deposit from Tangerine Chequing");

        apply_builtin_categorization(&mut conn).unwrap();
        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 1);
        assert_eq!(peer_of(&conn, "out1").as_deref(), Some("near"));
        assert!(peer_of(&conn, "far").is_none());

        // Re-run: nothing new, existing pair untouched.
        let n2 = pair_transfers(&mut conn).unwrap();
        assert_eq!(n2, 0);
        assert_eq!(peer_of(&conn, "out1").as_deref(), Some("near"));
    }

    #[test]
    fn ensure_default_categories_stamps_canonical_palette_colors() {
        // Regression: this path used to stamp '#94A3B8' grey for every category,
        // so an import-first user got indistinguishable chart colors that never
        // matched the canonical palette used everywhere else.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        ensure_default_categories(&mut conn).unwrap();
        let mut stmt = conn.prepare("SELECT id, color FROM categories").unwrap();
        let rows: Vec<(String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(!rows.is_empty());
        for (id, color) in rows {
            assert_eq!(
                color,
                crate::palette::color_for(&id),
                "category {id} must carry its canonical palette color"
            );
            assert_ne!(color, crate::palette::DEFAULT_COLOR, "{id} must not be grey");
        }
    }

    #[test]
    fn ensure_default_categories_never_overwrites_a_user_set() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn); // a partial user set exists
        let before: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap();
        ensure_default_categories(&mut conn).unwrap();
        let after: i64 = conn.query_row("SELECT COUNT(*) FROM categories", [], |r| r.get(0)).unwrap();
        assert_eq!(before, after, "must not seed defaults when a category set already exists");
    }
}
