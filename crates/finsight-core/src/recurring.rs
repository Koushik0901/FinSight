//! Recurring-transaction detection, redesigned (Phase 6 #9).
//!
//! The previous heuristic (`occurrences >= 2 AND avg_gap 5..400`) flagged any
//! merchant seen twice — so pay-per-use vendors (Uber Eats, EVO car share),
//! groceries (Walmart), dining (McDonald's, Dominos), transit and card payments
//! all became "subscriptions", while real subs (Spotify, OpenAI, Anthropic)
//! were buried.
//!
//! This module groups by a **normalized merchant**, then classifies each group
//! into a [`RecurringKind`] using multiple independent signals — cadence
//! regularity, amount stability within a tolerance band, minimum occurrences,
//! category exclusions, vendor hints, and transfer/payment detection — and
//! returns a **confidence** and human-readable **reasons** for each.

use crate::categorize::is_transfer;
use crate::error::CoreResult;
use crate::merchant::{
    bill_vendor_hint, canonical_merchant_key, is_membership_like, subscription_vendor_hint,
};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

/// Categories whose repeat purchases are almost never subscriptions/bills.
/// A vendor in these categories must be on the subscription allowlist to be
/// classified as a subscription.
const NON_SUBSCRIPTION_CATEGORIES: &[&str] =
    &["dining", "groceries", "transport", "shopping", "travel"];

/// Minimum occurrences before we call anything recurring.
const MIN_OCCURRENCES: usize = 3;
/// Amount tolerance band (fraction of the median) for "stable" amounts.
const AMOUNT_TOLERANCE: f64 = 0.15;
/// Gap coefficient-of-variation below this = "regular" cadence.
const REGULAR_GAP_CV: f64 = 0.40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecurringKind {
    /// Small/mid, regular, stable — a subscription (Spotify, ChatGPT…).
    Subscription,
    /// Regular obligation, possibly larger or band-variable (phone, utilities).
    Bill,
    /// Regular inflow (payroll, deposits).
    Income,
    /// Internal transfer / card payment / e-transfer — not a real recurring cost.
    Transfer,
    /// Repeats but irregular amount/cadence — groceries, dining, ride-hailing.
    RepeatPurchase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurringItem {
    /// Normalized grouping key.
    pub merchant_key: String,
    /// A representative raw descriptor for display.
    pub display_merchant: String,
    pub kind: RecurringKind,
    /// 0..1 confidence that this is a genuine recurring item of `kind`.
    pub confidence: f64,
    /// Human-readable evidence for the classification.
    pub reasons: Vec<String>,
    pub occurrences: i64,
    pub median_amount_cents: i64,
    pub last_amount_cents: i64,
    pub min_amount_cents: i64,
    pub max_amount_cents: i64,
    pub avg_gap_days: f64,
    /// 0..1, higher = more regular cadence.
    pub cadence_regularity: f64,
    /// 0..1, higher = more stable amount.
    pub amount_stability: f64,
    pub cadence: String,
    pub category_label: Option<String>,
    pub category_color: Option<String>,
    pub last_seen: String,
    pub next_expected: Option<String>,
}

struct Occurrence {
    date: chrono::NaiveDate,
    amount_cents: i64,
}

struct Group {
    key: String,
    display: String,
    occ: Vec<Occurrence>,
    category: Option<String>,
    category_color: Option<String>,
    any_transfer_flag: bool,
    any_membership_like: bool,
}

/// Detect recurring items from transaction history.
///
/// `window_days` bounds how far back (relative to the most recent transaction,
/// so historical imports still work) to consider. Returns items sorted by
/// descending confidence.
pub fn detect_recurring(conn: &Connection, window_days: i64) -> CoreResult<Vec<RecurringItem>> {
    // Anchor the window on the most recent transaction date, not wall-clock now,
    // so imported historical statements are still analyzed.
    let max_date: Option<String> = conn
        .query_row(
            "SELECT MAX(substr(posted_at,1,10)) FROM transactions",
            [],
            |r| r.get(0),
        )
        .ok()
        .flatten();
    let Some(max_date) = max_date else {
        return Ok(Vec::new());
    };
    let anchor = parse_date(&max_date).unwrap_or_else(|| chrono::Utc::now().date_naive());
    let cutoff = (anchor - chrono::Duration::days(window_days))
        .format("%Y-%m-%d")
        .to_string();

    // Investment-account rows are excluded: monthly TFSA contributions or
    // repeated BUY trades would otherwise read as recurring "bills".
    let mut stmt = conn.prepare(&format!(
        "SELECT t.merchant_raw, substr(t.posted_at,1,10) AS d, t.amount_cents, \
                c.label, c.color, COALESCE(t.is_transfer,0) \
         FROM transactions t LEFT JOIN categories c ON c.id = t.category_id \
         WHERE substr(t.posted_at,1,10) >= ?1 AND {} \
         ORDER BY t.posted_at ASC",
        crate::metrics::non_investment_txn_predicate("t")
    ))?;
    let rows = stmt.query_map([cutoff], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, i64>(2)?,
            r.get::<_, Option<String>>(3)?,
            r.get::<_, Option<String>>(4)?,
            r.get::<_, i64>(5)? != 0,
        ))
    })?;

    // Group by normalized merchant.
    let mut groups: std::collections::HashMap<String, Group> = std::collections::HashMap::new();
    for row in rows.flatten() {
        let (raw, date_str, amount, category, color, transfer_flag) = row;
        // Group by a VENDOR-CANONICAL key so brand/product variants of the same
        // subscription (OPENAI *CHATGPT SUBSCR / CHATGPT SUBSCRIPTION / OPENAI)
        // form one series instead of several sparse ones.
        let key = canonical_merchant_key(&raw);
        if key.is_empty() {
            continue;
        }
        let Some(date) = parse_date(&date_str) else {
            continue;
        };
        let g = groups.entry(key.clone()).or_insert_with(|| Group {
            key: key.clone(),
            display: raw.split("  ").next().unwrap_or(&raw).trim().to_string(),
            occ: Vec::new(),
            category: None,
            category_color: None,
            any_transfer_flag: false,
            any_membership_like: false,
        });
        g.occ.push(Occurrence { date, amount_cents: amount });
        if g.category.is_none() {
            g.category = category;
            g.category_color = color;
        }
        g.any_transfer_flag |= transfer_flag;
        // Check the FULL raw descriptor (not the canonical key, which strips
        // these very words) so an installment/membership fee is recognized even
        // when statement padding separates the vocabulary.
        g.any_membership_like |= is_membership_like(&raw.to_lowercase());
    }

    let mut out = Vec::new();
    for g in groups.into_values() {
        if g.occ.len() < MIN_OCCURRENCES {
            continue;
        }
        if let Some(item) = classify(g) {
            out.push(item);
        }
    }
    out.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
    Ok(out)
}

fn classify(g: Group) -> Option<RecurringItem> {
    let mut occ = g.occ;
    occ.sort_by_key(|o| o.date);
    let n = occ.len();

    // Gaps between consecutive occurrences.
    let gaps: Vec<f64> = occ
        .windows(2)
        .map(|w| (w[1].date - w[0].date).num_days() as f64)
        .filter(|&d| d > 0.0)
        .collect();
    if gaps.is_empty() {
        return None;
    }
    let mean_gap = mean(&gaps);
    let gap_cv = coefficient_of_variation(&gaps, mean_gap);
    let cadence_regularity = (1.0 - gap_cv.min(1.0)).max(0.0);
    // Cadence label, next-expected date, and the subscription/bill split use the
    // MEDIAN gap ("typical" spacing), not the mean, so a couple of billing lapses
    // don't drag a monthly subscription into the "quarterly" bucket (a real
    // ChatGPT sub that paused twice still reads monthly). The mean still drives
    // the regularity CV above.
    let avg_gap = median(&gaps);

    // Amount stats on absolute values.
    let abs_amounts: Vec<f64> = occ.iter().map(|o| o.amount_cents.abs() as f64).collect();
    let median_abs = median(&abs_amounts);
    let within_band = abs_amounts
        .iter()
        .filter(|&&a| median_abs > 0.0 && (a - median_abs).abs() / median_abs <= AMOUNT_TOLERANCE)
        .count();
    let amount_stability = within_band as f64 / n as f64;

    let median_amount_cents = median_signed(&occ);
    let min_amount_cents = occ.iter().map(|o| o.amount_cents).min().unwrap_or(0);
    let max_amount_cents = occ.iter().map(|o| o.amount_cents).max().unwrap_or(0);
    let last = occ.last().unwrap();
    let last_seen = last.date.format("%Y-%m-%d").to_string();
    let next_expected = if avg_gap > 0.0 {
        Some((last.date + chrono::Duration::days(avg_gap.round() as i64)).format("%Y-%m-%d").to_string())
    } else {
        None
    };
    let cadence = cadence_label(avg_gap).to_string();

    let positive = occ.iter().filter(|o| o.amount_cents > 0).count();
    let mostly_income = positive as f64 / n as f64 >= 0.6;
    let cat = g.category.clone();
    // When a transaction is not yet categorized, fall back to the deterministic
    // built-in categorizer so an uncategorized "OLIVE GARDEN" is still treated
    // as dining (and thus excluded from subscriptions), not a bill.
    let cat_key = cat
        .as_deref()
        .map(str::to_lowercase)
        .or_else(|| crate::categorize::builtin_category(&g.display).map(str::to_string));
    let excluded_category = cat_key
        .as_deref()
        .map(|c| NON_SUBSCRIPTION_CATEGORIES.contains(&c))
        .unwrap_or(false);

    let sub_hint = subscription_vendor_hint(&g.key);
    let bill_hint = bill_vendor_hint(&g.key);
    // A user-set REAL spending category (e.g. Housing on a recurring rent
    // e-transfer) is an explicit "this is a real cost" — respect it over the
    // transfer-descriptor keyword heuristic, so rent-by-e-transfer surfaces as a
    // recurring bill once categorized. A CONFIRMED transfer (`any_transfer_flag`)
    // is never overridden (those rows aren't categorized), and an uncategorized
    // transfer stays dismissed — so this can't turn a real internal transfer into
    // a fake bill.
    let user_categorized_real_cost = g
        .category
        .as_deref()
        .map_or(false, |c| !matches!(c.to_lowercase().as_str(), "transfer" | "transfers" | "income"));
    let looks_transfer = g.any_transfer_flag
        || (!user_categorized_real_cost && (is_transfer(&g.display) || is_payment_like(&g.key)));

    let mut reasons = vec![format!("{n} occurrences"), format!("~{cadence} cadence")];

    let (kind, mut confidence) = if looks_transfer {
        reasons.push("looks like a transfer / card payment / e-transfer".to_string());
        (RecurringKind::Transfer, 0.5)
    } else if mostly_income {
        reasons.push("mostly inflows (income/deposits)".to_string());
        (RecurringKind::Income, 0.5 + 0.4 * cadence_regularity)
    } else if let Some(v) = sub_hint {
        reasons.push(format!("known subscription vendor ({v})"));
        // Vendor-hinted subs still need some cadence regularity; amount may vary
        // (USD billing → FX), so we do not require tight amount stability here.
        (RecurringKind::Subscription, 0.6 + 0.35 * cadence_regularity)
    } else if let Some(v) = bill_hint {
        reasons.push(format!("known bill vendor ({v})"));
        (RecurringKind::Bill, 0.6 + 0.3 * cadence_regularity)
    } else if g.any_membership_like && cadence_regularity >= 0.5 {
        // A membership / installment / subscription fee is a real recurring
        // commitment even when the amount steps (e.g. an annual card fee billed
        // monthly that rises mid-year) — the amount-stability heuristic wrongly
        // buckets those as repeat purchases, so rescue by vocabulary. Small &
        // monthly-ish → subscription; larger / less frequent → bill.
        reasons.push("recurring membership / subscription / installment fee".to_string());
        if avg_gap <= 45.0 && median_abs <= 20_000.0 {
            (RecurringKind::Subscription, 0.55 + 0.3 * cadence_regularity)
        } else {
            (RecurringKind::Bill, 0.55 + 0.3 * cadence_regularity)
        }
    } else if excluded_category {
        // Dining/groceries/transport/shopping/travel without a subscription
        // vendor hint → a repeat purchase, never a subscription.
        reasons.push(format!(
            "category '{}' is a spending category, not a subscription",
            cat_key.as_deref().unwrap_or("?")
        ));
        (RecurringKind::RepeatPurchase, 0.3)
    } else if cadence_regularity >= (1.0 - REGULAR_GAP_CV) && amount_stability >= 0.6 {
        // Regular cadence + stable amount + not an excluded category → a genuine
        // recurring commitment. Small/monthly-ish → subscription, else bill.
        reasons.push("regular cadence and stable amount".to_string());
        if avg_gap <= 45.0 && median_abs <= 20_000.0 {
            (RecurringKind::Subscription, 0.5 + 0.25 * cadence_regularity + 0.15 * amount_stability)
        } else {
            (RecurringKind::Bill, 0.5 + 0.25 * cadence_regularity + 0.15 * amount_stability)
        }
    } else {
        // Repeats but irregular amount or cadence.
        if amount_stability < 0.6 {
            reasons.push("amount varies too much to be a subscription".to_string());
        }
        if cadence_regularity < (1.0 - REGULAR_GAP_CV) {
            reasons.push("timing is irregular".to_string());
        }
        (RecurringKind::RepeatPurchase, 0.25 + 0.2 * cadence_regularity)
    };

    // Occurrence bonus (more history = more confidence), capped.
    confidence += ((n.min(12) as f64 - 3.0) / 9.0).max(0.0) * 0.1;
    confidence = confidence.clamp(0.0, 0.99);

    Some(RecurringItem {
        merchant_key: g.key,
        display_merchant: g.display,
        kind,
        confidence,
        reasons,
        occurrences: n as i64,
        median_amount_cents,
        last_amount_cents: last.amount_cents,
        min_amount_cents,
        max_amount_cents,
        avg_gap_days: avg_gap,
        cadence_regularity,
        amount_stability,
        cadence,
        category_label: cat,
        category_color: g.category_color,
        last_seen,
        next_expected,
    })
}

/// Card-payment / transfer descriptors that may not carry an `is_transfer`
/// flag but must never be treated as bills/subscriptions.
fn is_payment_like(normalized: &str) -> bool {
    const PATTERNS: &[&str] = &[
        "payment received",
        "payment thank",
        "thank you",
        "e-transfer",
        "etransfer",
        "interac",
        "internet deposit",
        "internet withdrawal",
        "transfer to",
        "transfer from",
        "bill payment",
        "pre-authorized payment",
    ];
    PATTERNS.iter().any(|p| normalized.contains(p))
}

fn cadence_label(avg_gap: f64) -> &'static str {
    if avg_gap < 10.0 {
        "weekly"
    } else if avg_gap < 20.0 {
        "biweekly"
    } else if avg_gap < 45.0 {
        "monthly"
    } else if avg_gap < 100.0 {
        "quarterly"
    } else {
        "annual"
    }
}

fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

fn mean(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        0.0
    } else {
        xs.iter().sum::<f64>() / xs.len() as f64
    }
}

fn coefficient_of_variation(xs: &[f64], mean_val: f64) -> f64 {
    if xs.len() < 2 || mean_val == 0.0 {
        return 0.0;
    }
    let var = xs.iter().map(|x| (x - mean_val).powi(2)).sum::<f64>() / xs.len() as f64;
    var.sqrt() / mean_val
}

fn median(xs: &[f64]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    let mut v = xs.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = v.len() / 2;
    if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2.0
    } else {
        v[mid]
    }
}

fn median_signed(occ: &[Occurrence]) -> i64 {
    let mut v: Vec<i64> = occ.iter().map(|o| o.amount_cents).collect();
    v.sort_unstable();
    let mid = v.len() / 2;
    if v.is_empty() {
        0
    } else if v.len() % 2 == 0 {
        (v[mid - 1] + v[mid]) / 2
    } else {
        v[mid]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db::run_migrations, keychain, Db};
    use tempfile::TempDir;

    fn fresh() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let key = keychain::generate_random_key();
        let db = Db::open(&dir.path().join("rec.sqlcipher"), &key).unwrap();
        run_migrations(&db).unwrap();
        (dir, db)
    }

    fn seed_categories(conn: &Connection) {
        conn.execute("INSERT INTO category_groups(id,label,sort_order) VALUES('g','G',0)", []).unwrap();
        for (id, label) in [
            ("dining", "Dining"),
            ("groceries", "Groceries"),
            ("transport", "Transport"),
            ("subscriptions", "Subscriptions"),
            ("utilities", "Utilities"),
        ] {
            conn.execute(
                "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES(?1,'g',?2,'#888',0)",
                rusqlite::params![id, label],
            )
            .unwrap();
        }
        conn.execute("INSERT INTO accounts(id,owner,bank,type,name,currency,color,created_at) VALUES('a','me','B','Credit','Card','USD','#fff',datetime('now'))", []).unwrap();
    }

    /// Insert `count` charges every `gap` days from `start`, amount jittered by
    /// `jitter_frac`, with an optional category and transfer flag.
    fn insert_series(
        conn: &Connection,
        merchant: &str,
        start: &str,
        gap_days: i64,
        count: i64,
        base_cents: i64,
        jitter_frac: f64,
        category: Option<&str>,
        is_transfer: i64,
    ) {
        let start = chrono::NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap();
        for i in 0..count {
            let d = start + chrono::Duration::days(gap_days * i);
            let jitter = (base_cents as f64 * jitter_frac * ((i % 3) as f64 - 1.0)) as i64;
            let amt = base_cents + jitter;
            conn.execute(
                "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,category_id,is_transfer,status,created_at) \
                 VALUES(hex(randomblob(16)),'a',?1,?2,?3,?4,?5,'cleared',datetime('now'))",
                rusqlite::params![format!("{}T12:00:00Z", d.format("%Y-%m-%d")), amt, merchant, category, is_transfer],
            )
            .unwrap();
        }
    }

    fn find<'a>(items: &'a [RecurringItem], needle: &str) -> Option<&'a RecurringItem> {
        items.iter().find(|i| i.merchant_key.contains(needle))
    }

    #[test]
    fn categorized_rent_e_transfer_is_a_bill_not_dismissed_as_transfer() {
        // F3: rent paid by e-transfer, once categorized (Housing), is a real
        // recurring cost and must surface as a bill — keyed on the counterparty
        // so it's a distinct series, and NOT dismissed by the transfer keyword.
        // An UNcategorized recurring e-transfer stays dismissed.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        conn.execute(
            "INSERT INTO categories(id,group_id,label,color,sort_order) VALUES('housing','g','Housing','#888',0)",
            [],
        )
        .unwrap();

        insert_series(&conn, "Internet Banking E-TRANSFER 100000000001 Swathi", "2025-01-01", 30, 8, -160_000, 0.01, Some("housing"), 0);
        insert_series(&conn, "Internet Banking E-TRANSFER 200000000002 Landlord", "2025-01-05", 30, 8, -50_000, 0.01, None, 0);

        let items = detect_recurring(&conn, 400).unwrap();
        let rent = find(&items, "swathi").expect("categorized rent is a distinct, surfaced series");
        assert!(
            !matches!(rent.kind, RecurringKind::Transfer),
            "categorized rent must not be dismissed as a transfer (got {:?})",
            rent.kind
        );
        let other = find(&items, "landlord").expect("the other e-transfer is its own series");
        assert!(
            matches!(other.kind, RecurringKind::Transfer),
            "an UNcategorized recurring e-transfer stays dismissed as a transfer"
        );
    }

    #[test]
    fn real_subscriptions_are_detected_and_false_positives_are_not() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);

        // TRUE subscriptions — stable amount, monthly, subscription vendors.
        insert_series(&conn, "SPOTIFY                 STOCKHOLM", "2025-01-05", 30, 8, -716, 0.02, Some("subscriptions"), 0);
        // USD-billed sub: amount varies with FX (±10%), but vendor-hinted.
        insert_series(&conn, "OPENAI *CHATGPT SUBSCR  SAN FRANCISCO", "2025-01-10", 30, 6, -2900, 0.10, None, 0);
        insert_series(&conn, "ANTHROPIC               SAN FRANCISCO", "2025-02-01", 30, 5, -2940, 0.05, None, 0);
        insert_series(&conn, "FREEDOM MOBILE          877-946-3184", "2025-01-15", 30, 9, -4368, 0.0, Some("utilities"), 0);

        // FALSE positives — repeat purchases that must NOT be subscriptions.
        insert_series(&conn, "UBER EATS               TORONTO", "2025-01-02", 6, 20, -1500, 0.6, Some("dining"), 0);
        insert_series(&conn, "WALMART SUPERCENTER 121 BURNABY", "2025-01-03", 12, 10, -5000, 0.7, Some("groceries"), 0);
        insert_series(&conn, "EVO CAR SHARE           BURNABY", "2025-01-01", 7, 25, -800, 0.5, Some("transport"), 0);
        insert_series(&conn, "DOMINOS PIZZA 10082     BURNABY", "2025-01-04", 20, 4, -1200, 0.4, Some("dining"), 0);

        // Card payment / transfer — never a bill/subscription.
        insert_series(&conn, "PAYMENT RECEIVED - THANK YOU", "2025-01-06", 30, 6, 300000, 0.2, None, 0);
        insert_series(&conn, "Internet Withdrawal to Tangerine", "2025-01-08", 30, 4, -50000, 0.3, None, 1);

        let items = detect_recurring(&conn, 400).unwrap();

        // True subscriptions detected.
        for v in ["spotify", "openai", "anthropic"] {
            let it = find(&items, v).unwrap_or_else(|| panic!("{v} not detected"));
            assert_eq!(it.kind, RecurringKind::Subscription, "{v} should be a subscription (reasons: {:?})", it.reasons);
        }
        // Freedom Mobile → bill.
        assert_eq!(find(&items, "freedom mobile").unwrap().kind, RecurringKind::Bill);

        // False positives are NOT subscriptions/bills.
        for v in ["uber eats", "walmart", "evo car share", "dominos"] {
            let it = find(&items, v).unwrap_or_else(|| panic!("{v} missing"));
            assert!(
                matches!(it.kind, RecurringKind::RepeatPurchase),
                "{v} must be a repeat purchase, got {:?} ({:?})",
                it.kind,
                it.reasons
            );
        }

        // Card payment / transfer classified as such, never a bill/subscription.
        let pay = find(&items, "payment received").unwrap();
        assert!(
            matches!(pay.kind, RecurringKind::Transfer | RecurringKind::Income),
            "card payment must not be a bill/subscription, got {:?}",
            pay.kind
        );
        assert_eq!(find(&items, "internet withdrawal").unwrap().kind, RecurringKind::Transfer);

        // The "subscriptions" count that insights would show is now sane.
        let sub_count = items.iter().filter(|i| i.kind == RecurringKind::Subscription).count();
        assert!(sub_count >= 3 && sub_count <= 5, "expected ~3-4 subscriptions, got {sub_count}");
    }

    #[test]
    fn requires_minimum_occurrences() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        insert_series(&conn, "SPOTIFY  STOCKHOLM", "2025-01-05", 30, 2, -716, 0.0, Some("subscriptions"), 0);
        let items = detect_recurring(&conn, 400).unwrap();
        assert!(find(&items, "spotify").is_none(), "2 occurrences is not enough");
    }

    #[test]
    fn brand_variants_merge_into_one_monthly_series() {
        // P1-2: the three OpenAI descriptor variants each appear ~quarterly, but
        // interleaved they are one monthly subscription. Canonical grouping must
        // merge them into ONE series with a monthly cadence, not three sparse
        // "quarterly" ones.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        insert_series(&conn, "OPENAI *CHATGPT SUBSCR  SAN FRANCISCO", "2025-01-04", 90, 4, -2900, 0.05, None, 0);
        insert_series(&conn, "CHATGPT SUBSCRIPTION    SAN FRANCISCO", "2025-02-03", 90, 4, -2900, 0.05, None, 0);
        insert_series(&conn, "OPENAI                  SAN FRANCISCO", "2025-03-05", 90, 4, -2900, 0.05, None, 0);

        let items = detect_recurring(&conn, 500).unwrap();
        let openai: Vec<_> = items.iter().filter(|i| i.merchant_key == "openai").collect();
        assert_eq!(openai.len(), 1, "all OpenAI variants must merge into ONE series");
        let it = openai[0];
        assert_eq!(it.occurrences, 12, "all 12 charges land in one series");
        assert_eq!(it.kind, RecurringKind::Subscription);
        assert_eq!(
            it.cadence, "monthly",
            "interleaved variants read monthly, not quarterly (gap {:.1})",
            it.avg_gap_days
        );
    }

    #[test]
    fn membership_fee_with_price_step_is_recurring_not_repeat_purchase() {
        // P1-2: an annual card fee billed monthly whose price steps up mid-stream
        // (12.99 → 15.99). The higher price becomes the in-window majority, so the
        // 12.99 rows fall outside the amount-stability band → WITHOUT a vocabulary
        // rescue it is misclassified RepeatPurchase (the audit's conf-0.54 case).
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        insert_series(&conn, "MEMBERSHIP FEE INSTALLMENT", "2025-01-24", 30, 6, -1299, 0.0, None, 0);
        insert_series(&conn, "MEMBERSHIP FEE INSTALLMENT", "2025-07-24", 30, 8, -1599, 0.0, None, 0);

        let items = detect_recurring(&conn, 500).unwrap();
        let it = find(&items, "membership fee installment")
            .unwrap_or_else(|| panic!("membership fee not detected: {items:?}"));
        assert!(
            matches!(it.kind, RecurringKind::Subscription | RecurringKind::Bill),
            "a monthly membership fee must be a subscription/bill, got {:?} ({:?})",
            it.kind,
            it.reasons
        );
        assert_eq!(it.cadence, "monthly");
    }
}

