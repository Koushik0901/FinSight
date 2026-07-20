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

use crate::categorize::{is_nameless_bank_movement, is_transfer};
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
    // `is_nameless_bank_movement` reads the RAW descriptor, which matters
    // twice over: `canonical_merchant_key` strips transfer vocabulary as noise
    // AND keeps only the first three tokens, so testing the key alone asks
    // whether a string contains words that were removed from it before the
    // question was posed. "INTERNET BANKING INTERNET TRANSFER" canonicalizes to
    // "internet banking internet" — every transfer signal gone, and the ISP
    // vendor list's bare "internet" token left behind to promote it to a BILL.
    //
    // It tests for the ABSENCE OF A PAYEE rather than the presence of channel
    // words, because both an internal transfer and a utility bill paid through
    // online banking say "online banking". Only one of them names who was paid.
    let looks_transfer = g.any_transfer_flag
        || (!user_categorized_real_cost
            && (is_transfer(&g.display)
                || is_payment_like(&g.key)
                || is_nameless_bank_movement(&g.display)));

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
///
/// This is a RECURRING-CLASSIFICATION judgement, deliberately separate from
/// `categorize::is_transfer`. Transfer *detection* leaves a bare
/// "INTERNET TRANSFER" unflagged on purpose, so that pairing can match its two
/// legs (Rule 4) rather than flagging one side unilaterally. But for the
/// question "is this a recurring cost I should budget for?", a bank channel
/// word is never a payee — so the same descriptor that detection leaves open
/// is confidently NOT a bill here.
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

/// Confidence below which a detected item must not silently feed a forward-
/// looking projection — cashflow, surplus, "can I afford this", next month's
/// plan.
///
/// The detector reports a confidence for everything it finds, but every
/// consumer of it used to treat its own guess as settled fact. A merchant seen
/// three times at irregular intervals and a payroll deposit seen twenty-six
/// times on an exact biweekly cadence were quoted with identical certainty, and
/// a wrong "recurring" entry silently moved every projection built on it.
///
/// Gating PROJECTIONS is deliberately narrower than hiding the item: the
/// Recurring screen still lists low-confidence entries with their score and
/// reasons, because the user is the one who can confirm or dismiss them. What
/// they must not do is quietly become an obligation in arithmetic the user
/// never sees.
///
/// 0.6 matches [`crate::categorize`]'s sibling notion of "uncertain" and sits
/// above the cascade's `RepeatPurchase` band (0.25–0.30) and below a
/// vendor-recognised subscription (0.60+), so it separates "we recognised this"
/// from "this merely repeated".
pub const PROJECTION_CONFIDENCE_THRESHOLD: f64 = 0.6;

/// Average days per month — the same constant the cadence buckets are built
/// around, used to convert any cadence to a per-month figure.
const AVG_DAYS_PER_MONTH: f64 = 30.44;

impl RecurringItem {
    /// This item's cost per month, as a positive figure.
    ///
    /// Summing raw `last_amount_cents` across items answers a question nobody
    /// asked: it counts an annual insurance renewal at twelve times its monthly
    /// cost and a weekly charge at a quarter of it. Anything that compares
    /// recurring commitments against a MONTHLY surplus, budget, or plan has to
    /// normalize first, or the comparison is between different units.
    ///
    /// Prefers the classified `cadence` label and falls back to the observed
    /// gap, so an item whose cadence is unrecognised still contributes
    /// something proportional rather than its raw face value.
    pub fn monthly_equivalent_cents(&self) -> i64 {
        let abs = self.last_amount_cents.unsigned_abs() as f64;
        let per_month = match self.cadence.as_str() {
            "weekly" => abs * (AVG_DAYS_PER_MONTH / 7.0),
            "biweekly" => abs * (AVG_DAYS_PER_MONTH / 14.0),
            "monthly" => abs,
            "quarterly" => abs / 3.0,
            "annual" | "yearly" => abs / 12.0,
            _ if self.avg_gap_days > 0.0 => abs * (AVG_DAYS_PER_MONTH / self.avg_gap_days),
            _ => abs,
        };
        per_month.round() as i64
    }

    /// A recurring OUTFLOW the app is confident enough about to let it move a
    /// projection: a bill or subscription, above the confidence threshold.
    ///
    /// Income is excluded because it is not an obligation; transfers and repeat
    /// purchases are excluded because they are not recurring costs at all. That
    /// last exclusion is the one that matters most in practice — an internal
    /// transfer on a regular cadence looks exactly like a bill to a query that
    /// only groups by merchant and measures gaps, and was being counted as one.
    pub fn is_projection_obligation(&self) -> bool {
        matches!(self.kind, RecurringKind::Bill | RecurringKind::Subscription)
            && self.confidence >= PROJECTION_CONFIDENCE_THRESHOLD
            && self.last_amount_cents < 0
    }
}

/// Recurring outflows trustworthy enough to feed projections, most confident
/// first.
///
/// This exists so the several places that need "the user's recurring
/// obligations" ask one question instead of each writing its own SQL. Four
/// hand-rolled variants had drifted apart on transfer handling, investment
/// exclusion, sign, occurrence count, and cadence ceiling — the loosest of them
/// classified a bare internal transfer as a $1,000 biweekly bill.
pub fn projection_obligations(
    conn: &Connection,
    window_days: i64,
) -> CoreResult<Vec<RecurringItem>> {
    Ok(detect_recurring(conn, window_days)?
        .into_iter()
        .filter(RecurringItem::is_projection_obligation)
        .collect())
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

    // ── What may feed a projection ──────────────────────────────────────────

    /// A bank channel word is not a payee. "INTERNET TRANSFER" is what several
    /// banks call an online-banking money movement, and the ISP vendor list
    /// contains the bare token "internet" — so the classifier read a biweekly
    /// internal transfer as a high-confidence recurring BILL. Pinned directly
    /// on the classifier, not only on the downstream projection filter, so the
    /// fix cannot regress behind a filter that happens to mask it.
    #[test]
    fn an_online_banking_transfer_is_not_an_internet_bill() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        for descriptor in [
            "INTERNET BANKING INTERNET TRANSFER",
            "INTERNET TRANSFER 000000123456",
        ] {
            insert_series(&conn, descriptor, "2026-01-05", 14, 12, -100_000, 0.0, None, 0);
        }

        let items = detect_recurring(&conn, 395).unwrap();
        for item in &items {
            if item.display_merchant.to_lowercase().contains("internet") {
                assert_eq!(
                    item.kind,
                    RecurringKind::Transfer,
                    "bank channel wording must not read as an ISP bill: {item:?}"
                );
            }
        }

        // A genuine ISP bill must still be recognised — the fix must not have
        // been achieved by blinding the vendor list.
        insert_series(&conn, "TELUS COMMUNICATIONS", "2026-01-09", 30, 10, -8_500, 0.0, None, 0);
        let items = detect_recurring(&conn, 395).unwrap();
        let telus = find(&items, "telus").unwrap_or_else(|| panic!("telus missing: {items:?}"));
        assert_eq!(telus.kind, RecurringKind::Bill);
    }

    /// The opposite failure mode, and the reason the fix tests for a missing
    /// PAYEE rather than for channel words: a utility bill paid THROUGH online
    /// banking carries the same channel vocabulary as an internal transfer.
    /// Only one of them names who was paid.
    ///
    /// These descriptors are also the case a key-based check cannot see —
    /// `canonical_merchant_key` keeps three tokens, so "HYDRO ONE BILL PAYMENT"
    /// becomes "hydro one bill" and the payment wording never reaches it.
    #[test]
    fn a_bill_paid_through_online_banking_is_still_a_bill() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        insert_series(&conn, "ONLINE BANKING BILL PAYMENT HYDRO ONE", "2026-01-07", 30, 10, -14_200, 0.0, None, 0);
        insert_series(&conn, "INTERNET BANKING BILL PAYMENT TELUS", "2026-01-11", 30, 10, -9_300, 0.0, None, 0);
        // …alongside a genuinely nameless movement, so the two are separated by
        // the payee and not by anything incidental to the fixture.
        insert_series(&conn, "INTERNET BANKING INTERNET TRANSFER", "2026-01-05", 30, 10, -50_000, 0.0, None, 0);

        let items = detect_recurring(&conn, 395).unwrap();
        // Matched on the DISPLAY descriptor, not the canonical key: the key
        // keeps three tokens, so the payee these assertions are about
        // ("HYDRO", "TELUS") is not in it.
        for needle in ["HYDRO", "TELUS"] {
            let item = items
                .iter()
                .find(|i| i.display_merchant.contains(needle))
                .unwrap_or_else(|| panic!("{needle} missing entirely: {items:?}"));
            assert_ne!(
                item.kind,
                RecurringKind::Transfer,
                "a named payee makes this a real cost, not a transfer: {item:?}"
            );
            assert!(
                item.is_projection_obligation(),
                "and it must still be budgeted for: {item:?}"
            );
        }
        let nameless = items
            .iter()
            .find(|i| i.display_merchant.contains("INTERNET TRANSFER"))
            .unwrap_or_else(|| panic!("nameless movement missing: {items:?}"));
        assert_eq!(nameless.kind, RecurringKind::Transfer);
    }

    /// The failure the issue was filed about: a bare internal transfer on a
    /// regular cadence is indistinguishable from a bill to a query that only
    /// groups by merchant and measures gaps, and five such queries were each
    /// counting it as one.
    #[test]
    fn a_recurring_internal_transfer_never_becomes_a_projected_obligation() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        // A large, perfectly regular, uncategorized transfer — the shape most
        // likely to be mistaken for a serious bill.
        insert_series(
            &conn,
            "INTERNET BANKING INTERNET TRANSFER",
            "2026-01-05",
            14,
            12,
            -100_000,
            0.0,
            None,
            0,
        );
        insert_series(&conn, "SPOTIFY", "2026-01-03", 30, 8, -1_099, 0.0, None, 0);

        let obligations = projection_obligations(&conn, 395).unwrap();
        assert!(
            !obligations
                .iter()
                .any(|i| i.merchant_key.contains("internet")),
            "an internal transfer must not reach projections: {obligations:?}"
        );
        assert!(
            obligations.iter().any(|i| i.merchant_key.contains("spotify")),
            "a real subscription still does: {obligations:?}"
        );
    }

    #[test]
    fn income_and_repeat_purchases_are_not_obligations() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        // Payroll: regular, confident, but an INFLOW — not something to fund.
        insert_series(&conn, "ACME CORP PAYROLL", "2026-01-02", 14, 12, 250_000, 0.0, None, 0);
        // Groceries: repeats, but irregular amounts — a repeat purchase.
        insert_series(
            &conn,
            "SAVE ON FOODS",
            "2026-01-04",
            9,
            14,
            -12_000,
            0.6,
            Some("groceries"),
            0,
        );

        let obligations = projection_obligations(&conn, 395).unwrap();
        for item in &obligations {
            assert!(
                item.last_amount_cents < 0,
                "an inflow is not an obligation: {item:?}"
            );
            assert!(
                matches!(item.kind, RecurringKind::Bill | RecurringKind::Subscription),
                "only bills/subscriptions fund projections: {item:?}"
            );
            assert!(item.confidence >= PROJECTION_CONFIDENCE_THRESHOLD);
        }
        assert!(
            !obligations.iter().any(|i| i.merchant_key.contains("save on foods")),
            "irregular grocery spend is not a recurring obligation: {obligations:?}"
        );
    }

    /// Low confidence must SUPPRESS the projection without hiding the item —
    /// the user is the one who can confirm or dismiss it.
    #[test]
    fn a_weak_signal_stays_visible_but_does_not_feed_projections() {
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        // Three hits at wildly irregular gaps and amounts: enough to be
        // detected, nowhere near enough to budget against.
        let conn_ref = &conn;
        for (day, amt) in [("2026-01-03", -4_100), ("2026-02-19", -31_500), ("2026-04-27", -8_800)] {
            conn_ref
                .execute(
                    "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,is_transfer,status,created_at) \
                     VALUES(hex(randomblob(16)),'a',?1,?2,'ODD JOBS LTD',0,'cleared',datetime('now'))",
                    rusqlite::params![format!("{day}T12:00:00Z"), amt],
                )
                .unwrap();
        }

        let all = detect_recurring(&conn, 395).unwrap();
        let item = find(&all, "odd jobs")
            .unwrap_or_else(|| panic!("still detected and visible: {all:?}"));
        assert!(
            item.confidence < PROJECTION_CONFIDENCE_THRESHOLD,
            "an irregular series should not be confident: {item:?}"
        );
        assert!(
            !item.is_projection_obligation(),
            "and so must not feed projections"
        );
        assert!(
            !projection_obligations(&conn, 395)
                .unwrap()
                .iter()
                .any(|i| i.merchant_key.contains("odd jobs"))
        );
    }

    #[test]
    fn monthly_equivalent_amortizes_by_cadence_not_face_value() {
        let base = RecurringItem {
            merchant_key: "k".into(),
            display_merchant: "M".into(),
            kind: RecurringKind::Subscription,
            confidence: 0.9,
            reasons: vec![],
            occurrences: 4,
            median_amount_cents: -60_000,
            last_amount_cents: -60_000,
            min_amount_cents: -60_000,
            max_amount_cents: -60_000,
            avg_gap_days: 365.0,
            cadence_regularity: 1.0,
            amount_stability: 1.0,
            cadence: "annual".into(),
            category_label: None,
            category_color: None,
            last_seen: "2026-01-01".into(),
            next_expected: None,
        };
        // $600/year is $50/month of commitment — not $600.
        assert_eq!(base.monthly_equivalent_cents(), 5_000);

        let monthly = RecurringItem { cadence: "monthly".into(), avg_gap_days: 30.0, ..base.clone() };
        assert_eq!(monthly.monthly_equivalent_cents(), 60_000);

        let quarterly = RecurringItem { cadence: "quarterly".into(), avg_gap_days: 91.0, ..base.clone() };
        assert_eq!(quarterly.monthly_equivalent_cents(), 20_000);

        // Weekly costs MORE per month than its face value — the normalisation
        // has to run both directions, not just shrink big numbers.
        let weekly = RecurringItem {
            cadence: "weekly".into(),
            avg_gap_days: 7.0,
            last_amount_cents: -1_000,
            ..base.clone()
        };
        assert_eq!(weekly.monthly_equivalent_cents(), 4_349);

        // Unrecognised cadence falls back to the observed gap rather than the
        // raw face value.
        let odd = RecurringItem {
            cadence: "sporadic".into(),
            avg_gap_days: 60.0,
            last_amount_cents: -10_000,
            ..base.clone()
        };
        assert_eq!(odd.monthly_equivalent_cents(), 5_073);

        // No cadence and no gap: nothing to amortize by, so face value stands
        // rather than dividing by zero.
        let unknown = RecurringItem {
            cadence: String::new(),
            avg_gap_days: 0.0,
            last_amount_cents: -2_500,
            ..base
        };
        assert_eq!(unknown.monthly_equivalent_cents(), 2_500);
    }

    #[test]
    fn an_annual_renewal_does_not_dominate_a_monthly_obligation_total() {
        // The units bug in one number: a yearly renewal and a monthly sub, as
        // a monthly commitment figure.
        let (_d, db) = fresh();
        let conn = db.get().unwrap();
        seed_categories(&conn);
        insert_series(&conn, "SPOTIFY", "2026-01-03", 30, 12, -1_000, 0.0, None, 0);
        insert_series(&conn, "ANTHROPIC", "2023-02-10", 365, 4, -60_000, 0.0, None, 0);

        let monthly_total: i64 = projection_obligations(&conn, 2000)
            .unwrap()
            .iter()
            .map(|i| i.monthly_equivalent_cents())
            .sum();
        // $10/mo + $600/yr → $10 + $50, NOT $10 + $600.
        assert!(
            (5_500..=6_500).contains(&monthly_total),
            "annual renewal must be amortized, got {monthly_total}"
        );
    }
}

