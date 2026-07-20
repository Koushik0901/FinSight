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
    // ── P2-1: real merchant families seen uncategorized in the sample data ──
    // (airlines / car rental → travel; marketplace + electronics → shopping;
    // one more grocery/transit chain; LinkedIn → subscription). Evidence-based,
    // not test-case tuning: these are the coverable heads of the long tail.
    ("amzn", "shopping"), // "AMZN MKTP CA" — Amazon marketplace variant
    ("samsung", "shopping"),
    ("sobeys", "groceries"),
    ("presto", "transport"),
    ("linkedin", "subscriptions"),
    ("air india", "travel"),
    ("lufthansa", "travel"),
    ("cathay", "travel"),
    ("flair air", "travel"),
    ("makemytrip", "travel"),
    ("hertz", "travel"),
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
/// Merchant phrases that ALONE identify an internal money movement with high
/// precision — safe to flag even when the matching leg is not imported. Kept
/// deliberately TINY: only credit-card payments received on a card, and
/// explicit own-account markers. Broad, ambiguous phrases ("e-transfer",
/// "electronic funds transfer", "eft", bare "transfer to/from") are NOT here —
/// they also describe payroll, government benefits, and payments to real people
/// (rent!), so they only make a leg *eligible to pair* (see
/// `PAIRING_HINT_KEYWORDS`); a leg is flagged from them only when it actually
/// pairs with an equal-and-opposite leg in another account.
const UNILATERAL_TRANSFER_KEYWORDS: &[&str] = &[
    // Credit-card payment received on a card (reduces card debt — never income).
    "payment received - thank you",
    "payment - thank you",
    "payment thank you", // CIBC "PAYMENT THANK YOU/PAIEMENT MERCI"
    "paiement merci",
    "autopay",
    // Explicit own-account markers (the string names the account moved to/from).
    "internet withdrawal to",  // Tangerine internal out
    "internet deposit from",   // Tangerine internal in
    "transfer to account",
    "transfer from account",
    "internal transfer",
    "online banking transfer",
    "tfr-to",
    "tfr-from",
];

/// Broad transfer vocabulary that makes a leg ELIGIBLE to pair with an
/// equal-and-opposite leg in another account, but never flags it alone. A
/// superset of the unilateral list plus the ambiguous phrases that also occur
/// on income (payroll/benefits) — those only become transfers when paired.
const PAIRING_HINT_KEYWORDS: &[&str] = &[
    "transfer",
    "e-transfer",
    "e transfer",
    "email money transfer",
    "electronic funds transfer",
    "eft",
    "preauthorized debit",
    "pre-authorized debit",
    "preauthorized payment",
    "fulfill request", // CIBC "FULFILL REQUEST" = an e-transfer request fulfilled
    "withdrawal to",
    "deposit from",
    "bill payment",
    "money transfer",
    "wire",
];

/// True when a merchant string looks like an internal money movement (credit-
/// card payment or an explicitly own-account transfer) with enough precision to
/// exclude it from income/spending even without a matching leg. Ambiguous
/// transfer phrasing is handled by pairing, not here.
pub fn is_transfer(merchant_raw: &str) -> bool {
    let m = merchant_raw.to_lowercase();
    UNILATERAL_TRANSFER_KEYWORDS.iter().any(|kw| m.contains(kw))
        // A row that both carries transfer vocabulary AND names an own account
        // or card ("INTERNET TRANSFER <ref> TO ACCOUNT 04930", "… TO CARD 4505")
        // is an internal move regardless of whether the other leg is imported.
        || (is_pairing_eligible(merchant_raw) && has_own_account_marker(merchant_raw))
}

/// True when a brokerage activity type (V048 `transactions.activity_type`,
/// stored provider-verbatim from investment CSV imports) marks the row as an
/// internal move rather than income/spending: a Trade converts cash↔security
/// inside the account, a MoneyMovement is a contribution/withdrawal whose
/// other leg lives in another account. Dividend/Interest (income) and Tax
/// (expense) stay out of this list deliberately.
pub fn activity_implies_transfer(activity_type: &str) -> bool {
    matches!(activity_type, "Trade" | "MoneyMovement")
}

/// True when a merchant string carries any transfer vocabulary — the leg is a
/// candidate to be paired with its opposite. Broader (lower precision) than
/// `is_transfer`; only ever used together with an equal-and-opposite match.
fn is_pairing_eligible(merchant_raw: &str) -> bool {
    let m = merchant_raw.to_lowercase();
    PAIRING_HINT_KEYWORDS.iter().any(|kw| m.contains(kw))
        || has_cc_counterparty_hint(merchant_raw)
}

/// Vocabulary for the transfer-review surface: rows that LOOK like a money
/// transfer but were neither unilaterally flagged nor paired, so their
/// income/expense treatment is a silent guess until the user rules on them
/// (bare "INTERNET TRANSFER <ref>" legs whose counter-leg was never imported,
/// person-to-person e-transfers that may be rent, gifts, or reimbursements).
/// Deliberately NARROWER than `PAIRING_HINT_KEYWORDS`: bill payments,
/// preauthorized debits, and — critically — payroll/benefits ("Electronic
/// Funds Transfer PAY Wage/salary", "… DEPOSIT AE/EI", direct deposits) are
/// almost always real money, and burying the genuine suspects under every
/// paycheck would make the review list useless (measured on samples/: a bare
/// "transfer" keyword put $95k of wages at the top of the list).
pub const TRANSFER_REVIEW_KEYWORDS: &[&str] = &[
    "internet transfer",
    "e-transfer",
    "e transfer",
    "etransfer",
    "money transfer",
    "tfr-",
    "fulfill request",
];

/// SQL predicate selecting every leg that could involve a *person* — the
/// broader sibling of [`transfer_review_predicate`].
///
/// The review predicate deliberately narrows to rows still needing a verdict:
/// unflagged, unpaired, uncategorized. That is right for a triage queue and
/// wrong for a running tab, because a leg the user already ruled on still
/// moved money. "Am I up or down with this person" has to count every leg,
/// settled or not.
///
/// Still excludes investment-account rows, which are internal by construction
/// and never a payment to a person.
pub fn counterparty_candidate_predicate(alias: &str) -> String {
    let vocab = TRANSFER_REVIEW_KEYWORDS
        .iter()
        .map(|kw| format!("lower({alias}.merchant_raw) LIKE '%{kw}%'"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let non_investment = crate::metrics::non_investment_txn_predicate(alias);
    format!("({alias}.amount_cents != 0 AND {non_investment} AND ({vocab}))")
}

/// SQL predicate selecting the transactions that need a user's transfer
/// verdict. `alias` is the `transactions` table alias in the caller's query.
/// Built from `TRANSFER_REVIEW_KEYWORDS` so the vocabulary lives in one place;
/// the keywords are static lowercase strings (enforced by a unit test), so
/// interpolation is safe.
pub fn transfer_review_predicate(alias: &str) -> String {
    let vocab = TRANSFER_REVIEW_KEYWORDS
        .iter()
        .map(|kw| format!("lower({alias}.merchant_raw) LIKE '%{kw}%'"))
        .collect::<Vec<_>>()
        .join(" OR ");
    // Investment-account rows never need a verdict: they are already excluded
    // from income/expense wholesale, so an "is this a transfer?" answer would
    // change nothing — don't ask.
    let non_investment = crate::metrics::non_investment_txn_predicate(alias);
    format!(
        "({alias}.is_transfer = 0 AND {alias}.transfer_peer_id IS NULL \
         AND {alias}.transfer_override IS NULL AND {alias}.category_id IS NULL \
         AND {alias}.amount_cents != 0 AND {non_investment} AND ({vocab}))"
    )
}

/// An explicit own-account marker: the string names an account or card the
/// money moved to/from, strongly implying an internal move (vs a payment to a
/// person). Lifts a pair over the precision bar when no shared reference exists.
fn has_own_account_marker(merchant_raw: &str) -> bool {
    let m = merchant_raw.to_lowercase();
    const MARKERS: &[&str] = &[
        "to account",
        "from account",
        "to card",
        "from card",
        "internet withdrawal to",
        "internet deposit from",
        "transfer to account",
        "transfer from account",
        "internal transfer",
    ];
    MARKERS.iter().any(|kw| m.contains(kw))
}

/// Long digit runs (≥6) in a merchant string — bank transaction reference
/// numbers. The SAME reference on two equal-and-opposite legs in different
/// accounts is a near-certain internal transfer (astronomically unlikely to
/// collide by chance), so a shared token is sufficient to pair. Shorter runs
/// (account-number fragments like "00930") are excluded to avoid collisions.
fn reference_tokens(merchant_raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in merchant_raw.chars() {
        if ch.is_ascii_digit() {
            cur.push(ch);
        } else if cur.len() >= 6 {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.clear();
        }
    }
    if cur.len() >= 6 {
        out.push(cur);
    }
    out
}

/// Public aliases for how Canadian banks describe themselves on the OTHER
/// bank's statement (e.g. a Tangerine deposit from a CIBC account reads "from
/// CANADIAN IMPERIAL"). This is generic public banking knowledge, not
/// sample-specific data. Each entry maps a substring that may appear in the
/// user's `accounts.bank` value to the distinctive descriptor fragments other
/// banks print — truncation-robust (statements clip long names).
const BANK_ALIASES: &[(&str, &[&str])] = &[
    ("cibc", &["cibc", "canadian imp"]),
    ("tangerine", &["tangerine"]),
    ("rbc", &["rbc", "royal bank"]),
    ("royal bank", &["rbc", "royal bank"]),
    ("td", &["td canada", "toronto domin", "td bank"]),
    ("toronto", &["td canada", "toronto domin"]),
    ("bmo", &["bmo", "bank of montr"]),
    ("montreal", &["bmo", "bank of montr"]),
    ("scotia", &["scotiabank", "bank of nova", "nova scotia"]),
    ("simplii", &["simplii"]),
    ("national bank", &["national bank", "banque national"]),
    ("desjardins", &["desjardins"]),
    ("amex", &["amex", "american express"]),
    ("american express", &["amex", "american express"]),
    ("wealthsimple", &["wealthsimple"]),
    ("eq bank", &["eq bank"]),
    ("laurentian", &["laurentian"]),
];

/// Generic name tokens that must never count as an "owner name" match — they
/// carry no identity (an account owner of "You" or "Household" is not a name
/// that would appear in an e-transfer descriptor).
const GENERIC_OWNER_TOKENS: &[&str] = &[
    "you", "self", "household", "joint", "family", "shared", "and", "the",
    "account", "chequing", "checking", "savings", "credit", "card",
];

/// The user's own identity, derived from their accounts + household members, so
/// self-transfers can be recognised even when the two legs use different
/// mechanisms (an e-transfer out of CIBC arriving as an "EFT Deposit from
/// CANADIAN IMPERIAL" in Tangerine, or an Interac e-transfer to one's own
/// name). Derived from the user's OWN data — never hard-coded merchant strings.
#[derive(Debug, Clone, Default)]
pub struct TransferContext {
    /// Each owner's significant lowercase name tokens (generic tokens dropped).
    owner_name_tokens: Vec<Vec<String>>,
    /// Distinctive descriptor fragments for every bank the user holds an account
    /// at (via `BANK_ALIASES`), so "from <another of my banks>" reads as self.
    owned_bank_fragments: Vec<String>,
}

impl TransferContext {
    /// Load owner names (accounts + household members) and owned-bank aliases.
    pub fn load(conn: &Connection) -> CoreResult<Self> {
        let mut owner_names: Vec<String> = Vec::new();
        {
            let mut stmt =
                conn.prepare("SELECT DISTINCT owner FROM accounts WHERE owner IS NOT NULL")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows {
                owner_names.push(r?);
            }
        }
        if table_exists(conn, "household_members")? {
            let mut stmt = conn.prepare("SELECT name FROM household_members")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows {
                owner_names.push(r?);
            }
        }
        let owner_name_tokens: Vec<Vec<String>> = owner_names
            .iter()
            .map(|n| name_tokens(n))
            .filter(|t| !t.is_empty())
            .collect();

        let mut banks: Vec<String> = Vec::new();
        {
            let mut stmt =
                conn.prepare("SELECT DISTINCT lower(bank) FROM accounts WHERE bank IS NOT NULL")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            for r in rows {
                banks.push(r?);
            }
        }
        let mut owned_bank_fragments: Vec<String> = Vec::new();
        for bank in &banks {
            for (needle, fragments) in BANK_ALIASES {
                if bank.contains(needle) {
                    for f in *fragments {
                        if !owned_bank_fragments.iter().any(|x| x == f) {
                            owned_bank_fragments.push((*f).to_string());
                        }
                    }
                }
            }
        }
        Ok(Self {
            owner_name_tokens,
            owned_bank_fragments,
        })
    }

    fn descriptor_names_owner(&self, lowered: &str) -> bool {
        self.owner_name_tokens.iter().any(|tokens| {
            let hits = tokens.iter().filter(|t| lowered.contains(*t)).count();
            // A single-token name needs that token; a multi-token name needs ≥2
            // to avoid a shared first name matching a friend.
            if tokens.len() == 1 {
                hits == 1
            } else {
                hits >= 2
            }
        })
    }

    fn descriptor_names_owned_bank(&self, lowered: &str) -> bool {
        self.owned_bank_fragments.iter().any(|f| lowered.contains(f))
    }

    /// True when a row is an internal move to/from one of the user's OWN
    /// accounts (named owner or another owned bank) AND carries transfer
    /// vocabulary — high precision, so it excludes income like payroll/benefits
    /// (which name neither the owner nor another of the user's banks).
    pub(crate) fn is_self_transfer(&self, merchant_raw: &str) -> bool {
        if !is_pairing_eligible(merchant_raw) {
            return false;
        }
        let m = merchant_raw.to_lowercase();
        self.descriptor_names_owner(&m) || self.descriptor_names_owned_bank(&m)
    }
}

/// Significant lowercase tokens of a person's name (≥3 chars, generic words
/// dropped) for owner-name matching.
fn name_tokens(name: &str) -> Vec<String> {
    name.split(|c: char| !c.is_alphanumeric())
        .map(|t| t.to_lowercase())
        .filter(|t| t.len() >= 3 && !GENERIC_OWNER_TOKENS.contains(&t.as_str()))
        .collect()
}

/// True for a *nameless* bank transfer that carries only a numeric reference —
/// the bank's internal-transfer product ("Internet Banking INTERNET TRANSFER
/// 000000135957"), never a payment to a named person. It is pairing-eligible
/// (transfer vocabulary), has a bank reference number, no own-account marker,
/// and — crucially — no counterparty NAME (every alphabetic ≥3-char token is a
/// structural transfer word). Each bank stamps its own reference, so the two
/// legs of one such transfer carry DIFFERENT refs and no account marker: neither
/// the shared-reference rule nor the own-account rule can pair them. But a
/// nameless transfer is internal by the bank's product definition, so two
/// equal-and-opposite such legs are safe to pair (see `pair_transfers` Rule 4).
fn is_bare_reference_transfer(merchant_raw: &str) -> bool {
    if !is_nameless_bank_movement(merchant_raw) || has_own_account_marker(merchant_raw) {
        return false;
    }
    !reference_tokens(merchant_raw).is_empty() // needs a bank reference to key on
}

/// True when a descriptor is transfer vocabulary and NOTHING ELSE — a bank
/// product name with no counterparty. "INTERNET BANKING INTERNET TRANSFER" is
/// one; "ONLINE BANKING BILL PAYMENT HYDRO ONE" is not, because `hydro` names
/// who was paid.
///
/// The distinction is what separates a money movement from a bill paid THROUGH
/// online banking. A plain substring test cannot make it: both contain channel
/// words, and the payee may sit beyond the three tokens
/// `canonical_merchant_key` keeps, so it is invisible to anything working from
/// the key. Requiring every alphabetic token to be structural asks the right
/// question — is there a payee here at all?
///
/// Unlike [`is_bare_reference_transfer`] this does not require a reference
/// number, because a nameless transfer is nameless whether or not the bank
/// stamped a number on it. Pairing needs the number (it has to key on
/// something); classification does not.
pub fn is_nameless_bank_movement(merchant_raw: &str) -> bool {
    if !is_pairing_eligible(merchant_raw) {
        return false;
    }
    merchant_raw
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && t.chars().all(|c| c.is_alphabetic()))
        .all(|t| TRANSFER_STRUCTURAL_TOKENS.contains(&t.to_lowercase().as_str()))
}

/// Structural (non-identifying) tokens kept verbatim when redacting a transfer
/// descriptor for the cloud LLM — bank/product/direction words. Anything else
/// alphabetic in a named-transfer string is a counterparty NAME and is dropped.
pub(crate) const TRANSFER_STRUCTURAL_TOKENS: &[&str] = &[
    "internet", "banking", "interac", "transfer", "email", "money", "fulfill",
    "request", "electronic", "funds", "eft", "payment", "paiement", "merci",
    "deposit", "withdrawal", "preauthorized", "authorized", "debit", "credit",
    "account", "card", "to", "from", "thank", "you", "received", "e", "pre",
    "bill", "pay", "wire", "online", "branch", "transaction",
    // Compound tokens whose alphabetic core (hyphen removed) must be kept.
    "etransfer", "preauthorized", "emt", "etfr",
];

/// The pattern to propose for an "always categorize like this" rule from a
/// user's categorization.
///
/// For a normal merchant the raw string is the right key — matching future
/// identical rows. But a person-to-person transfer descriptor
/// ("Internet Banking E-TRANSFER 106001023942 Swathi") carries a UNIQUE reference
/// number every time, so a rule keyed on the raw string would only ever match
/// that one row — the single most important recurring cost, rent-by-e-transfer,
/// would never stick. So for a transfer/e-transfer descriptor, generalize to
/// `%<counterparty tokens>%` (structural transfer words and reference numbers
/// stripped) so one confirmation catches every payment to that person. Normal
/// merchants are returned unchanged. The user always confirms the proposed
/// pattern, so a generalized key is never applied silently.
pub fn suggested_rule_pattern(merchant_raw: &str) -> String {
    let lower = merchant_raw.to_lowercase();
    let is_transferish = ["e-transfer", "etransfer", "e transfer", "interac"]
        .iter()
        .any(|k| lower.contains(k));
    if !is_transferish {
        return merchant_raw.to_string();
    }
    // Keep only counterparty NAME tokens: alphabetic, ≥3 chars, and not a
    // structural transfer word (bank/product/direction vocabulary).
    let name: Vec<String> = merchant_raw
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && t.chars().all(|c| c.is_alphabetic()))
        .map(|t| t.to_lowercase())
        .filter(|t| !TRANSFER_STRUCTURAL_TOKENS.contains(&t.as_str()))
        .collect();
    if name.is_empty() {
        return merchant_raw.to_string();
    }
    format!("%{}%", name.join(" "))
}

/// Mask personally-identifying tokens from a merchant string BEFORE it is sent
/// to a cloud LLM for categorization: (1) bank reference / account / phone
/// numbers (digit runs ≥ 4) become `#`, and (2) in a named e-transfer / Interac
/// / money-transfer descriptor, the counterparty's NAME is dropped. The
/// category-relevant vocabulary ("E-TRANSFER", "PAYMENT") is preserved — only
/// the identity is removed. Non-transfer merchants (a store name) are unchanged.
pub fn redact_for_llm(merchant_raw: &str) -> String {
    // 1) Mask long digit runs everywhere.
    let mut masked = String::new();
    let mut digits = String::new();
    let flush = |digits: &mut String, out: &mut String| {
        if digits.len() >= 4 {
            out.push('#');
        } else {
            out.push_str(digits);
        }
        digits.clear();
    };
    for ch in merchant_raw.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else {
            flush(&mut digits, &mut masked);
            masked.push(ch);
        }
    }
    flush(&mut digits, &mut masked);

    // 2) Drop counterparty names from person-to-person transfer descriptors.
    let lower = masked.to_lowercase();
    let named_transfer = ["e-transfer", "e transfer", "interac", "email money transfer", "fulfill request"]
        .iter()
        .any(|k| lower.contains(k));
    if !named_transfer {
        return masked;
    }
    let keep: std::collections::HashSet<&str> = TRANSFER_STRUCTURAL_TOKENS.iter().copied().collect();
    let out: Vec<String> = masked
        .split_whitespace()
        .filter(|tok| {
            let core: String = tok.chars().filter(|c| c.is_alphabetic()).collect();
            // Keep punctuation/masked/number-only tokens and known structural
            // words; drop any other alphabetic token (a name).
            core.is_empty() || keep.contains(core.to_lowercase().as_str())
        })
        .map(|s| s.to_string())
        .collect();
    let joined = out.join(" ");
    if joined.trim().is_empty() {
        "E-TRANSFER".to_string()
    } else {
        joined
    }
}

fn table_exists(conn: &Connection, name: &str) -> CoreResult<bool> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
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
    ref_tokens: Vec<String>,
    eligible: bool,
    own_account: bool,
    bare_ref: bool,
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
               AND COALESCE(t.transfer_override, 1) != 0 \
             ORDER BY t.posted_at, t.amount_cents, t.merchant_raw, t.id",
        )?;
        let rows = stmt.query_map([], |r| {
            let merchant_raw: String = r.get(5)?;
            Ok(PairCandidate {
                id: r.get(0)?,
                account_id: r.get(1)?,
                account_type: r.get(2)?,
                day: r.get(3)?,
                amount_cents: r.get(4)?,
                flagged: r.get::<_, i64>(6)? != 0,
                ref_tokens: reference_tokens(&merchant_raw),
                eligible: is_pairing_eligible(&merchant_raw),
                own_account: has_own_account_marker(&merchant_raw),
                bare_ref: is_bare_reference_transfer(&merchant_raw),
                merchant_raw,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };
    // Keep only rows that could participate at all: anything flagged, or carrying
    // any transfer vocabulary. Pure spending/income rows are dropped — they can
    // never be a transfer leg — which also keeps the O(n^2) match cheap.
    candidates.retain(|c| c.flagged || c.eligible);

    let eligible = |a: &PairCandidate, b: &PairCandidate| -> bool {
        if a.account_id == b.account_id {
            return false;
        }
        // Legs must be exactly equal-and-opposite (no fee tolerance: a missed
        // transfer is far cheaper than falsely deleting real income/spending).
        if a.amount_cents != -b.amount_cents {
            return false;
        }
        if (a.day - b.day).abs() > PAIR_WINDOW_DAYS {
            return false;
        }
        // Rule 1 — shared reference token. The same bank reference number on two
        // equal-and-opposite legs in different accounts is a near-certain
        // internal transfer; sufficient on its own (catches CIBC/internal
        // transfers whose phrasing interposes the reference number).
        if a.ref_tokens.iter().any(|t| b.ref_tokens.contains(t)) {
            return true;
        }
        // Rule 2 — credit-card payment ↔ its bank-side debit. One leg is a
        // card-payment on a Credit account; the other carries a card/bill hint.
        let cc_pair = |x: &PairCandidate, y: &PairCandidate| {
            x.account_type == "Credit"
                && is_cc_payment(&x.merchant_raw)
                && (has_cc_counterparty_hint(&y.merchant_raw) || y.eligible)
        };
        if cc_pair(a, b) || cc_pair(b, a) {
            return true;
        }
        // Rule 3 — both legs carry transfer vocabulary AND at least one names an
        // own account/card ("… TO ACCOUNT", "Internet Withdrawal to …"). The
        // own-account marker is what separates an internal move from a
        // coincidental income/expense collision, so payroll/benefits (no marker)
        // are never eaten.
        if a.eligible && b.eligible && (a.own_account || b.own_account) {
            return true;
        }
        // Rule 4 — both legs are NAMELESS reference-only transfers (transfer
        // vocabulary + a numeric bank reference, no counterparty name and no
        // own-account marker). Each bank stamps its own reference, so their refs
        // differ (Rule 1 can't match) and there's no account marker (Rule 3
        // can't) — the real-data gap where bare "INTERNET TRANSFER <ref>" legs
        // leaked. A nameless transfer is internal by the bank's product
        // definition, so an equal-and-opposite pair across accounts is safe to
        // flag; a NAMED e-transfer (rent/income) fails `bare_ref` and is spared.
        if a.bare_ref && b.bare_ref {
            return true;
        }
        false
    };

    // Greedy nearest-date matching. Candidates are ordered by stable CONTENT
    // (posted_at, amount, merchant) before id, so both the iteration order and
    // the equal-distance tie-break (first-in-order wins) are reproducible even
    // when row ids are assigned randomly on re-import — id is only a final
    // tie-break among rows identical in every content field, where the choice
    // of which leg to flag is immaterial.
    let mut used: HashSet<usize> = HashSet::new();
    let mut pairs: Vec<(String, String)> = Vec::new();
    for i in 0..candidates.len() {
        if used.contains(&i) {
            continue;
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
/// so they slot into the conscious-spending breakdown.
/// `(id, group_id, label, spending_type)` — spending_type is Ramit Sethi's
/// Conscious Spending Plan bucket (`fixed` | `investments` | `savings` |
/// `guilt_free`) that powers the Budget "Spending mix" breakdown. Essentials
/// you can't easily opt out of are `fixed`; discretionary lifestyle spending
/// is `guilt_free`. None of the starter categories are savings/investments —
/// those flows are transfers, not spending categories.
const DEFAULT_CATEGORIES: &[(&str, &str, &str, &str)] = &[
    ("dining", "daily", "Dining", "guilt_free"),
    ("groceries", "daily", "Groceries", "fixed"),
    ("transport", "daily", "Transport", "fixed"),
    ("shopping", "lifestyle", "Shopping", "guilt_free"),
    ("travel", "lifestyle", "Travel", "guilt_free"),
    ("gifts", "lifestyle", "Gifts", "guilt_free"),
    ("housing", "fixed", "Housing", "fixed"),
    ("utilities", "fixed", "Utilities", "fixed"),
    ("subscriptions", "fixed", "Subscriptions", "fixed"),
    ("health", "wellbeing", "Health", "fixed"),
];

/// The canonical conscious-spending bucket for a starter category id, or
/// `None` for unknown/custom categories (those stay untagged until the user
/// decides — a wrong guess is worse than an honest blank).
pub fn default_spending_type(id: &str) -> Option<&'static str> {
    DEFAULT_CATEGORIES
        .iter()
        .find(|(cid, _, _, _)| *cid == id)
        .map(|(_, _, _, st)| *st)
}
const DEFAULT_GROUPS: &[(&str, &str)] = &[
    ("fixed", "Fixed"),
    ("daily", "Daily"),
    ("lifestyle", "Lifestyle"),
    ("wellbeing", "Wellbeing"),
];

/// Seed the investing group + categories used by activity-driven
/// categorization (brokerage CSV imports). Unlike `ensure_default_categories`
/// this must work for users with an established category set, so it inserts
/// just these rows with INSERT OR IGNORE — and is only called when an import
/// actually produced investment-income/tax rows, never speculatively.
/// Deliberately NOT part of DEFAULT_CATEGORIES: users without investment
/// accounts should never see these.
pub fn ensure_investment_categories(conn: &mut Connection) -> CoreResult<()> {
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT OR IGNORE INTO category_groups(id, label, sort_order) VALUES('investing', 'Investing', 50)",
        [],
    )?;
    // Dividends + interest are income (spending_type stays NULL — the
    // conscious-spending buckets classify spending, not income); withholding
    // tax is a real cost you can't opt out of → `fixed`.
    tx.execute(
        "INSERT OR IGNORE INTO categories(id, group_id, label, color, spending_type, sort_order) \
         VALUES('investment-income', 'investing', 'Investment income', ?1, NULL, 100)",
        params![crate::palette::color_for("investment-income")],
    )?;
    tx.execute(
        "INSERT OR IGNORE INTO categories(id, group_id, label, color, spending_type, sort_order) \
         VALUES('withholding-tax', 'investing', 'Withholding tax', ?1, 'fixed', 101)",
        params![crate::palette::color_for("withholding-tax")],
    )?;
    tx.commit()?;
    Ok(())
}

/// Activity-driven category for investment rows (checked before the merchant
/// keyword map): dividends and interest are investment income, NRT & friends
/// are withholding tax. Trade/MoneyMovement never reach this — they are
/// transfers and the categorizer skips them structurally.
fn activity_category(activity_type: &str) -> Option<&'static str> {
    match activity_type {
        "Dividend" | "Interest" => Some("investment-income"),
        "Tax" => Some("withholding-tax"),
        _ => None,
    }
}

/// Investment income is never a user-managed budget envelope — it's pure
/// income (spending_type stays NULL, see `ensure_investment_categories`), so
/// it can never carry a sensible "spend" and would otherwise sit permanently
/// in a Budget page's unbudgeted list with no meaningful action to take.
pub fn is_budgetable_category(category_id: &str) -> bool {
    category_id != "investment-income"
}

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
    for (i, (id, group_id, label, spending_type)) in DEFAULT_CATEGORIES.iter().enumerate() {
        tx.execute(
            "INSERT OR IGNORE INTO categories(id, group_id, label, color, spending_type, sort_order) \
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, group_id, label, crate::palette::color_for(id), spending_type, i as i64],
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
    // The user's own identity (owner names + owned-bank aliases) lets us treat a
    // move to/from one of their OWN accounts as a transfer even when the two
    // legs use different mechanisms and never share a reference number.
    let transfer_ctx = TransferContext::load(conn)?;
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
    // Also select the current `is_transfer` so the hot loop can skip a write
    // when the flag is unchanged (the common case: a non-transfer stays 0).
    type PendingRow = (
        String,
        String,
        bool,
        bool,
        bool,
        Option<bool>,
        Option<String>,
    );
    let pending: Vec<PendingRow> = {
        let mut stmt = conn.prepare(
            "SELECT id, merchant_raw, category_id IS NULL, transfer_peer_id IS NOT NULL, is_transfer, transfer_override, activity_type \
             FROM transactions \
             WHERE category_id IS NULL OR is_transfer = 1",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)? != 0,
                r.get::<_, i64>(3)? != 0,
                r.get::<_, i64>(4)? != 0,
                r.get::<_, Option<i64>>(5)?.map(|v| v != 0),
                r.get::<_, Option<String>>(6)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };

    // Lazily seed the investing categories the moment an import actually
    // carries dividend/interest/tax rows to file under them (the empty-table
    // guard in `ensure_default_categories` can't help existing users).
    let mut existing = existing;
    let needs_investment_categories = pending.iter().any(|(_, _, uncategorized, _, _, _, at)| {
        *uncategorized
            && at
                .as_deref()
                .and_then(activity_category)
                .is_some_and(|cat| !existing.contains(cat))
    });
    if needs_investment_categories {
        ensure_investment_categories(conn)?;
        existing.insert("investment-income".to_string());
        existing.insert("withholding-tax".to_string());
    }

    let now = chrono::Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    let mut count: u32 = 0;
    // Prepare the hot-loop statements once and reuse the cached handles for
    // every row. `execute` re-parses the SQL on each call; with thousands of
    // rows per import that per-row recompilation dominates — caching the
    // statements removes it (the same class of win that sped CSV import up).
    {
        let mut set_transfer = tx
            .prepare_cached("UPDATE transactions SET is_transfer = ?1 WHERE id = ?2")?;
        let mut set_category = tx.prepare_cached(
            "UPDATE transactions SET category_id = ?1, ai_confidence = NULL, ai_explanation = NULL WHERE id = ?2",
        )?;
        let mut record_categorization = tx.prepare_cached(
            "INSERT INTO categorizations(id, txn_id, category_id, source, confidence, model, at) \
             VALUES(?1, ?2, ?3, 'builtin', 1.0, NULL, ?4)",
        )?;
        for (txn_id, merchant, uncategorized, paired, currently_transfer, override_, activity) in
            pending
        {
            // Transfer detection runs regardless of category state, and is written
            // in BOTH directions so a re-run after the keyword list changes corrects
            // stale flags (e.g. an "Interac - Purchase" no longer treated as a
            // transfer once the over-broad 'interac' keyword was removed).
            // EXCEPT: a leg paired to a peer transaction (`pair_transfers`) is a
            // transfer by construction — rule B pairs legs whose merchants carry no
            // transfer keyword, and un-flagging them here would undo the pairing.
            // ALSO EXCEPT: a user verdict (`transfer_override`) always wins — a
            // re-run must never overwrite what the user explicitly decided.
            // Activity typing (Trade/MoneyMovement from brokerage imports) is
            // checked FIRST: those rows carry no transfer keyword in their
            // merchant ("Buy ACME"), and without this the bidirectional write
            // would silently UN-flag them on the next re-run.
            let unilateral_transfer = activity
                .as_deref()
                .map(activity_implies_transfer)
                .unwrap_or(false)
                || is_transfer(&merchant)
                || transfer_ctx.is_self_transfer(&merchant);
            if !paired && override_.is_none() {
                // Only write when the flag actually flips — the overwhelming
                // majority of rows are non-transfers that already read 0.
                if unilateral_transfer != currently_transfer {
                    set_transfer.execute(params![unilateral_transfer as i64, txn_id])?;
                }
            }
            if !uncategorized {
                continue;
            }
            // Invariant: transfers are never categorized (see docs + memory). In
            // practice transfer keywords and the category keyword map are disjoint,
            // but make it structural for paired legs, whose merchants CAN look like
            // ordinary bill payments. A user's "this IS a transfer" verdict blocks
            // categorization the same way; "this is NOT a transfer" makes the row
            // categorizable even when its descriptor looks like a transfer.
            let treat_as_transfer = override_.unwrap_or(paired || unilateral_transfer);
            if treat_as_transfer {
                continue;
            }
            // Activity typing beats the merchant keyword map: a "Dividend —
            // ACME" row is investment income by construction, not a guess.
            let Some(cat) = activity
                .as_deref()
                .and_then(activity_category)
                .or_else(|| builtin_category(&merchant))
            else {
                continue;
            };
            if !existing.contains(cat) {
                continue;
            }
            set_category.execute(params![cat, txn_id])?;
            record_categorization.execute(params![
                Uuid::new_v4().to_string(),
                txn_id,
                cat,
                now
            ])?;
            count += 1;
        }
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
    fn investment_income_is_not_budgetable() {
        assert!(!is_budgetable_category("investment-income"));
    }

    #[test]
    fn withholding_tax_and_ordinary_categories_are_budgetable() {
        assert!(is_budgetable_category("withholding-tax"));
        assert!(is_budgetable_category("groceries"));
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
        // Unilaterally safe: credit-card payments received on a card, and
        // explicit own-account markers.
        assert!(is_transfer("PAYMENT RECEIVED - THANK YOU"));
        assert!(is_transfer("PAYMENT THANK YOU/PAIEMENT MERCI"));
        assert!(is_transfer("Internet Deposit from Tangerine"));
        assert!(is_transfer("Internet Withdrawal to Tangerine"));
        // CIBC's "INTERNET TRANSFER <ref> TO ACCOUNT" / "TO CARD" names an own
        // account or card, so it is an internal move even if the other leg is
        // not imported (transfer vocab + own-account marker).
        assert!(is_transfer("Internet Banking INTERNET TRANSFER 000000112252 TO ACCOUNT 04930"));
        assert!(is_transfer("Internet Banking INTERNET TRANSFER 000000227766 TO CARD 4505"));
        // But bare "to account" with no transfer vocabulary must NOT trip it.
        assert!(!is_transfer("REFUND CREDITED TO ACCOUNT HOLDER"));

        // An e-transfer to a PERSON is NOT a transfer on its own — it may be
        // rent or a payment to a friend. It only becomes a transfer if a
        // matching opposite leg is imported (see pair_* tests). This was the
        // bug that made rent-via-e-transfer invisible in spending.
        assert!(!is_transfer("INTERAC e-Transfer To: Koushik C"));
        assert!(!is_transfer("INTERAC e-Transfer From: BRITISH"));
        assert!(!is_transfer("Email Money Transfer to Alice"));
        assert!(!is_transfer("Internet Banking E-TRANSFER 011630 SREE VYSHNAVI"));
        // Payroll and government benefits ride "Electronic Funds Transfer
        // DEPOSIT" too — must never be unilaterally flagged as a transfer.
        assert!(!is_transfer("Electronic Funds Transfer DEPOSIT 387402_260630 Infoblox"));
        assert!(!is_transfer("Electronic Funds Transfer DEPOSIT AE/EI"));

        // Real spending must NOT be flagged as a transfer.
        assert!(!is_transfer("TIM HORTONS #3356 BURNABY"));
        assert!(!is_transfer("WALMART SUPERCENTER BURNABY"));
        assert!(!is_transfer("Interest Paid"));
        // "Interac" is Canada's debit network — a debit PURCHASE or a fee is not
        // a transfer.
        assert!(!is_transfer("Interac - Purchase - COSTCO WHOLESALE W51"));
        assert!(!is_transfer("Interac Network Usage Charge"));
    }

    #[test]
    fn suggested_rule_pattern_generalizes_e_transfers_but_not_normal_merchants() {
        // A rent e-transfer's reference number changes every month; a rule must
        // key on the COUNTERPARTY so one confirmation catches every future payment
        // to that person (F3: rent-by-e-transfer becomes visible and sticky).
        assert_eq!(
            suggested_rule_pattern("Internet Banking E-TRANSFER 106001023942 Swathi"),
            "%swathi%"
        );
        assert_eq!(suggested_rule_pattern("INTERAC e-Transfer To: Koushik"), "%koushik%");
        // Normal merchants keep the exact string (unchanged behavior).
        assert_eq!(suggested_rule_pattern("AMAZON.CA"), "AMAZON.CA");
        assert_eq!(
            suggested_rule_pattern("TIM HORTONS #3356 BURNABY"),
            "TIM HORTONS #3356 BURNABY"
        );
        // A bare internal internet transfer has no counterparty name → not
        // generalized (correct: it isn't a payment to a person).
        assert_eq!(
            suggested_rule_pattern("Internet Banking INTERNET TRANSFER 000000239758"),
            "Internet Banking INTERNET TRANSFER 000000239758"
        );
    }

    #[test]
    fn redact_for_llm_strips_names_and_reference_numbers() {
        // Person-to-person e-transfer: name AND reference number removed.
        let r = redact_for_llm("Internet Banking E-TRANSFER 011654884429 swathi");
        assert!(!r.to_lowercase().contains("swathi"), "name must be dropped: {r}");
        assert!(!r.contains("011654884429"), "reference number masked: {r}");
        assert!(r.to_lowercase().contains("transfer"), "category vocab kept: {r}");

        let r2 = redact_for_llm("INTERAC e-Transfer From: SATHVIK DIVILI");
        assert!(!r2.to_uppercase().contains("SATHVIK"));
        assert!(!r2.to_uppercase().contains("DIVILI"));

        // Ordinary merchants keep their NAME (only digit runs are masked).
        let tim = redact_for_llm("TIM HORTONS #3356 BURNABY");
        assert!(tim.contains("TIM HORTONS") && tim.contains("BURNABY"), "{tim}");
        assert!(!tim.contains("3356"), "store number masked: {tim}");
        assert_eq!(redact_for_llm("STARBUCKS 12345678 SEATTLE"), "STARBUCKS # SEATTLE");
        // A non-transfer with a person-looking token is NOT a named transfer, so
        // it is left intact (we never touch normal merchant identities).
        assert_eq!(redact_for_llm("PAYPAL SOMECORP"), "PAYPAL SOMECORP");
    }

    #[test]
    fn reference_tokens_extracts_long_runs_only() {
        assert_eq!(
            reference_tokens("INTERNET TRANSFER 000000238417 FROM ACCOUNT 00930/****233"),
            vec!["000000238417".to_string()],
            "12-digit ref kept; 5-digit account fragment dropped"
        );
        assert!(reference_tokens("TIM HORTONS #3356 BURNABY").is_empty());
    }

    #[test]
    fn pairs_unflagged_internal_transfers_by_shared_reference() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
                   ('sav','You','CIBC','Savings','Sav','CAD','#222','manual',datetime('now'))",
            [],
        )
        .unwrap();
        // Same 12-digit reference on both legs; NO own-account marker and no
        // unilateral keyword — so only the shared reference can pair them.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('a','chk','2026-05-01T12:00:00Z',-20000,'Internet Banking INTERNET TRANSFER 000000238417','cleared',datetime('now')),\
             ('b','sav','2026-05-01T12:00:00Z', 20000,'Internet Banking INTERNET TRANSFER 000000238417','cleared',datetime('now'))",
            [],
        )
        .unwrap();
        assert!(!is_transfer("Internet Banking INTERNET TRANSFER 000000238417"),
            "a bare internet-transfer with no own-account marker is not unilaterally flagged");
        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 1, "the two legs pair via their shared reference number");
        let flagged: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions WHERE is_transfer=1 AND transfer_peer_id IS NOT NULL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(flagged, 2, "both legs flagged as a transfer once paired");
    }

    #[test]
    fn pairs_bare_internet_transfers_with_differing_references() {
        // F0 real-data leak: each bank stamps its OWN reference, so the two legs
        // of one INTERNET TRANSFER carry DIFFERENT refs — Rule 1 (shared ref)
        // can't match, and with no own-account marker Rule 3 can't either. A
        // nameless INTERNET TRANSFER is an internal move by the bank's product
        // definition, so an equal-and-opposite pair is safe to flag (Rule 4).
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
                   ('sav','You','CIBC','Savings','Sav','CAD','#222','manual',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('a','chk','2026-05-01T12:00:00Z',-200000,'Internet Banking INTERNET TRANSFER 000000135957','cleared',datetime('now')),\
             ('b','sav','2026-05-02T12:00:00Z', 200000,'Internet Banking INTERNET TRANSFER 000000220329','cleared',datetime('now'))",
            [],
        )
        .unwrap();
        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 1, "bare equal-and-opposite internet transfers pair despite differing refs");
        let flagged: i64 = conn
            .query_row("SELECT COUNT(*) FROM transactions WHERE is_transfer=1 AND transfer_peer_id IS NOT NULL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(flagged, 2, "both bare legs flagged once paired");
    }

    #[test]
    fn bare_transfer_pairing_does_not_eat_a_named_e_transfer() {
        // A bare INTERNET TRANSFER out must NOT pair with an equal-and-opposite
        // e-transfer that NAMES a person — that could be rent/income, not an
        // internal move. Rule 4 only pairs two NAMELESS reference-only transfers.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
                   ('sav','You','CIBC','Savings','Sav','CAD','#222','manual',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('a','chk','2026-05-01T12:00:00Z',-200000,'Internet Banking INTERNET TRANSFER 000000135957','cleared',datetime('now')),\
             ('b','sav','2026-05-01T12:00:00Z', 200000,'Internet Banking E-TRANSFER 000000999888 Swathi','cleared',datetime('now'))",
            [],
        )
        .unwrap();
        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 0, "a nameless transfer does not pair with a NAMED e-transfer of the opposite amount");
    }

    #[test]
    fn pairing_is_deterministic_regardless_of_row_id_order() {
        // The audit probe re-imports with random UUIDs, so pairing must not
        // depend on which id a row happens to receive. Three same-date
        // equal-and-opposite bare transfers make the greedy match ambiguous
        // (the -$2000 leg can pair with either +$2000 leg); the outcome must be
        // decided by stable content (amount, then merchant), never by row id.
        fn run(ids: [&str; 3]) -> Vec<String> {
            let (_d, db) = fresh_db();
            let mut conn = db.get().unwrap();
            seed_categories(&conn);
            conn.execute(
                "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES\
                 ('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
                 ('sav','You','CIBC','Savings','Sav','CAD','#222','manual',datetime('now')),\
                 ('tng','You','Tangerine','Savings','Tng','CAD','#333','manual',datetime('now'))",
                [],
            )
            .unwrap();
            conn.execute(
                &format!(
                    "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
                     ('{}','chk','2026-05-01T12:00:00Z',-200000,'Internet Banking INTERNET TRANSFER 000000000001','cleared',datetime('now')),\
                     ('{}','sav','2026-05-01T12:00:00Z', 200000,'Internet Banking INTERNET TRANSFER 000000000002','cleared',datetime('now')),\
                     ('{}','tng','2026-05-01T12:00:00Z', 200000,'Internet Banking INTERNET TRANSFER 000000000003','cleared',datetime('now'))",
                    ids[0], ids[1], ids[2]
                ),
                [],
            )
            .unwrap();
            pair_transfers(&mut conn).unwrap();
            let mut flagged: Vec<String> = conn
                .prepare("SELECT merchant_raw FROM transactions WHERE is_transfer=1")
                .unwrap()
                .query_map([], |r| r.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect();
            flagged.sort();
            flagged
        }
        // Same content, opposite id orderings → identical flagged set.
        let ascending = run(["id-a", "id-b", "id-c"]);
        let descending = run(["id-z", "id-y", "id-x"]);
        assert_eq!(ascending.len(), 2, "exactly one pair forms from three ambiguous legs");
        assert_eq!(
            ascending, descending,
            "pairing outcome must be identical regardless of row-id ordering"
        );
    }

    #[test]
    fn e_transfer_naming_a_household_member_is_a_self_transfer() {
        // Money moving to a household member (e.g. a partner) stays WITHIN the
        // household, so their e-transfer must read as an internal transfer, not
        // spending — but ONLY for a registered member. An e-transfer to a friend
        // who is not in the household stays real spending.
        let (_d, db) = fresh_db();
        let conn = db.get().unwrap();
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','Jordan Michael Avery','CIBC','Checking','Chq','CAD','#111','manual',datetime('now'))",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO household_members(id,name,color,created_at) \
             VALUES('m-partner','Robin','#abc',datetime('now'))",
            [],
        )
        .unwrap();
        let ctx = TransferContext::load(&conn).unwrap();
        assert!(
            ctx.is_self_transfer("Internet Banking E-TRANSFER 011654884429 Robin"),
            "an e-transfer naming a registered household member is an internal move"
        );
        assert!(
            !ctx.is_self_transfer("Internet Banking E-TRANSFER 011654884429 Casey"),
            "an e-transfer to a non-member stays real spending"
        );
        // The account owner's full name still needs ≥2 tokens: a friend who
        // merely shares the owner's first name must NOT be swallowed.
        assert!(
            !ctx.is_self_transfer("Internet Banking E-TRANSFER 011654884429 Jordan"),
            "a lone first-name match on a multi-token owner is not enough"
        );
    }

    #[test]
    fn pairing_skips_user_declared_non_transfers() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) \
             VALUES('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
                   ('sav','You','CIBC','Savings','Sav','CAD','#222','manual',datetime('now'))",
            [],
        )
        .unwrap();
        // Identical to the shared-reference pair above, except the user has
        // already ruled one leg is real spending — nothing may pair with it.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at,transfer_override) VALUES\
             ('a','chk','2026-05-01T12:00:00Z',-20000,'Internet Banking INTERNET TRANSFER 000000238417','cleared',datetime('now'),0),\
             ('b','sav','2026-05-01T12:00:00Z', 20000,'Internet Banking INTERNET TRANSFER 000000238417','cleared',datetime('now'),NULL)",
            [],
        )
        .unwrap();
        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 0, "a user-declared non-transfer is excluded from pairing");
    }

    #[test]
    fn transfer_review_keywords_are_sql_safe_and_lowercase() {
        // `transfer_review_predicate` interpolates these into a LIKE pattern;
        // they must be static lowercase literals with no quotes or wildcards.
        for kw in TRANSFER_REVIEW_KEYWORDS {
            assert_eq!(*kw, kw.to_lowercase(), "keyword must be lowercase: {kw}");
            for bad in ['\'', '%', '_', '"'] {
                assert!(!kw.contains(bad), "keyword {kw:?} contains SQL-unsafe {bad:?}");
            }
        }
    }

    #[test]
    fn does_not_pair_coincidental_income_and_expense() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        conn.execute(
            "INSERT INTO accounts(id,owner,bank,type,name,currency,color,source,created_at) VALUES\
             ('chk','You','CIBC','Checking','Chq','CAD','#111','manual',datetime('now')),\
             ('sav','You','Tangerine','Savings','Sav','CAD','#222','manual',datetime('now'))",
            [],
        )
        .unwrap();
        // Payroll deposit and an unrelated equal purchase in another account,
        // same day — no shared ref, no own-account marker → must NOT pair.
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,created_at) VALUES\
             ('pay','chk','2026-05-01T12:00:00Z', 118600,'Electronic Funds Transfer DEPOSIT AE/EI','cleared',datetime('now')),\
             ('buy','sav','2026-05-01T12:00:00Z',-118600,'FLAIR AIRLINES YXE','cleared',datetime('now'))",
            [],
        )
        .unwrap();
        let n = pair_transfers(&mut conn).unwrap();
        assert_eq!(n, 0, "real income must never pair with a coincidental expense");
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
    fn ensure_default_categories_stamps_conscious_spending_types() {
        // The Budget "Spending mix" is powered by spending_type; the seeder must
        // assign the canonical bucket so it works without manual tagging.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        ensure_default_categories(&mut conn).unwrap();
        let read = |id: &str| -> Option<String> {
            conn.query_row(
                "SELECT spending_type FROM categories WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(read("housing").as_deref(), Some("fixed"));
        assert_eq!(read("groceries").as_deref(), Some("fixed"));
        assert_eq!(read("dining").as_deref(), Some("guilt_free"));
        assert_eq!(read("travel").as_deref(), Some("guilt_free"));
        assert_eq!(default_spending_type("not-a-starter"), None, "custom categories stay untagged");
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

    // ---- activity-aware categorization (brokerage CSV imports) ----

    fn insert_activity_txn(
        conn: &Connection,
        id: &str,
        merchant: &str,
        activity_type: &str,
        is_transfer: bool,
    ) {
        conn.execute(
            "INSERT INTO transactions(id,account_id,posted_at,amount_cents,merchant_raw,status,is_anomaly,created_at,is_transfer,activity_type) \
             VALUES(?1,'a1','2024-01-01T00:00:00Z',1500,?2,'cleared',0,'2024-01-01T00:00:00Z',?3,?4)",
            params![id, merchant, is_transfer as i64, activity_type],
        )
        .unwrap();
    }

    #[test]
    fn rerun_does_not_unflag_activity_transfers() {
        // The bidirectional transfer write (which un-flags rows whose merchant
        // lost its keyword) must never strip Trade/MoneyMovement rows: their
        // merchants ("Buy ACME") carry no transfer vocabulary at all.
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        insert_activity_txn(&conn, "t1", "Buy ACME", "Trade", true);
        insert_activity_txn(&conn, "t2", "Transfer in (EFT)", "MoneyMovement", true);

        for _ in 0..2 {
            apply_builtin_categorization(&mut conn).unwrap();
            for id in ["t1", "t2"] {
                let flagged: i64 = conn
                    .query_row(
                        "SELECT is_transfer FROM transactions WHERE id = ?1",
                        params![id],
                        |r| r.get(0),
                    )
                    .unwrap();
                assert_eq!(flagged, 1, "{id} must stay a transfer across re-runs");
            }
        }
        // And transfers are never categorized.
        let cat: Option<String> = conn
            .query_row(
                "SELECT category_id FROM transactions WHERE id = 't1'",
                params![],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cat, None);
    }

    #[test]
    fn dividend_interest_tax_categorized_and_categories_lazily_seeded() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn); // established user set WITHOUT investing categories

        insert_activity_txn(&conn, "d1", "Dividend — ACME", "Dividend", false);
        insert_activity_txn(&conn, "i1", "Interest", "Interest", false);
        insert_activity_txn(&conn, "x1", "Withholding tax (NRT)", "Tax", false);
        apply_builtin_categorization(&mut conn).unwrap();

        let cat = |id: &str| -> Option<String> {
            conn.query_row(
                "SELECT category_id FROM transactions WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap()
        };
        assert_eq!(cat("d1").as_deref(), Some("investment-income"));
        assert_eq!(cat("i1").as_deref(), Some("investment-income"));
        assert_eq!(cat("x1").as_deref(), Some("withholding-tax"));

        // None of them count as transfers — they stay in income/expense.
        let transfers: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM transactions WHERE is_transfer = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(transfers, 0);
    }

    #[test]
    fn investment_categories_not_seeded_without_investment_rows() {
        let (_d, db) = fresh_db();
        let mut conn = db.get().unwrap();
        seed_categories(&conn);
        insert_txn(&conn, "t1", "STARBUCKS #123");
        apply_builtin_categorization(&mut conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM categories WHERE id IN ('investment-income','withholding-tax')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0, "investing categories must only appear when needed");
    }
}
