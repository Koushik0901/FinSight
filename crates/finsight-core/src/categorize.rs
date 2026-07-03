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
    "e-transfer",              // INTERAC e-Transfer to/from self
    "e transfer",
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

/// Apply the built-in keyword categorizer to every currently-uncategorized
/// transaction. Only assigns categories that exist in the `categories` table.
/// Returns the number of transactions categorized. Idempotent: a second run
/// touches nothing, because matched rows are no longer `category_id IS NULL`.
pub fn apply_builtin_categorization(conn: &mut Connection) -> CoreResult<u32> {
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

    // Consider every transaction that is either uncategorized OR not yet
    // transfer-flagged, so a re-run after this feature ships back-fills existing
    // rows too.
    let pending: Vec<(String, String, bool)> = {
        let mut stmt = conn.prepare(
            "SELECT id, merchant_raw, category_id IS NULL FROM transactions \
             WHERE category_id IS NULL OR is_transfer = 0",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? != 0))
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
    for (txn_id, merchant, uncategorized) in pending {
        // Transfer detection runs regardless of category state.
        if is_transfer(&merchant) {
            tx.execute(
                "UPDATE transactions SET is_transfer = 1 WHERE id = ?1",
                params![txn_id],
            )?;
        }
        if !uncategorized {
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
        // Real spending must NOT be flagged as a transfer.
        assert!(!is_transfer("TIM HORTONS #3356 BURNABY"));
        assert!(!is_transfer("WALMART SUPERCENTER BURNABY"));
        assert!(!is_transfer("Interest Paid"));
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
    fn flags_transfers_even_when_no_categories_exist_yet() {
        // Onboarding imports before categories are committed; transfer flagging
        // must still work so report totals are correct in that window.
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
        assert_eq!(n, 0, "no categories exist, so nothing is categorized");
        let is_tf: i64 = conn
            .query_row("SELECT is_transfer FROM transactions WHERE id='t1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(is_tf, 1, "transfer still flagged without categories");
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
}
