//! Merchant normalization — a shared primitive used by categorization,
//! recurring detection, and insights so that "UBER EATS   HTTPS://HELP.UB",
//! "UBER EATS   TORONTO", and "UBER EATS" all group to the same vendor.
//!
//! The goal is *grouping stability*, not a pretty display name: two charges
//! from the same vendor should normalize to the same key even when the raw
//! descriptor carries location, store number, URL, or payment-processor noise.

/// Normalize a raw merchant descriptor into a stable grouping key.
///
/// Generic rules (no per-merchant hardcoding):
/// 1. Fixed-width bank statements pad the merchant name and append a
///    location/URL; take the text before the first run of 2+ spaces.
/// 2. Strip common payment-processor prefixes (`paypal *`, `sq *`, `tst-`,
///    `bam*`, `pp*`) that vary the descriptor for the same vendor.
/// 3. Remove URLs, phone numbers, and long digit/ref runs (store numbers).
/// 4. Lowercase, drop non-alphanumeric noise, collapse whitespace.
pub fn normalize_merchant(raw: &str) -> String {
    // 1. Take the segment before the first run of 2+ spaces (statement padding).
    let head = split_on_double_space(raw);

    let mut s = head.to_lowercase();

    // 2. Strip known payment-processor prefixes (the processor name precedes the
    //    real vendor, e.g. "PAYPAL *STARBUCKS"). We only strip an explicit
    //    allowlist so we never accidentally drop the vendor itself (e.g. the
    //    "OPENAI" in "OPENAI *CHATGPT").
    for prefix in ["paypal *", "paypal*", "sq *", "sq*", "tst-", "tst*", "bam*", "pp*", "pos "] {
        if let Some(stripped) = s.strip_prefix(prefix) {
            s = stripped.trim_start_matches('*').trim().to_string();
        }
    }

    // 3. Tokenize (treat '*' as a separator so sub-tags like "*chatgpt" split
    //    into their own token) and drop noise tokens (URLs, phones, ref runs).
    let mut tokens: Vec<String> = Vec::new();
    for tok in s.split(|c: char| c.is_whitespace() || c == '/' || c == ',' || c == '*') {
        let tok = tok.trim_matches(|c: char| !c.is_alphanumeric());
        if tok.is_empty() {
            continue;
        }
        if is_noise_token(tok) {
            continue;
        }
        tokens.push(tok.to_string());
    }

    // Keep at most the first 3 meaningful tokens — enough to identify a vendor
    // ("uber eats", "openai chatgpt subscr") without trailing location tokens
    // that some formats append without padding.
    tokens.truncate(3);
    let joined = tokens.join(" ");
    if joined.is_empty() {
        // Fall back to a cleaned version of the whole descriptor.
        head.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        joined
    }
}

/// A human-facing merchant label: the segment before the first run of 2+
/// spaces (statement city/padding), trimmed. Used for display next to a
/// canonical key. Falls back to the whole string.
pub fn split_display(raw: &str) -> String {
    raw.split("  ").next().unwrap_or(raw).trim().to_string()
}

fn split_on_double_space(raw: &str) -> &str {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b' ' && bytes[i + 1] == b' ' {
            return raw[..i].trim();
        }
        i += 1;
    }
    raw.trim()
}

fn is_noise_token(tok: &str) -> bool {
    // URLs and hosts.
    if tok.starts_with("http") || tok.contains("www") || tok.contains(".com") || tok.contains(".ca")
    {
        return true;
    }
    // Phone-number-ish (mostly digits + separators handled by tokenizer already).
    let digit_count = tok.chars().filter(|c| c.is_ascii_digit()).count();
    let len = tok.chars().count();
    // A run that is mostly digits (store numbers, refs, phone fragments).
    if len >= 3 && digit_count * 2 >= len {
        return true;
    }
    false
}

/// A short lowercase key of known subscription/SaaS/telecom vendors. Used to
/// allow-list vendors that should count as subscriptions even when a strict
/// heuristic might otherwise drop them, and to *rescue* real subscriptions from
/// the "repeat purchase" bucket. Matching is substring-based on the normalized
/// merchant. This is a hint, not the sole signal.
pub fn subscription_vendor_hint(normalized: &str) -> Option<&'static str> {
    const VENDORS: &[&str] = &[
        "spotify",
        "netflix",
        "disney",
        "crave",
        "youtube premium",
        "apple.com/bill",
        "apple music",
        "icloud",
        "amazon prime",
        "prime video",
        "openai",
        "chatgpt",
        "anthropic",
        "claude",
        "openrouter",
        "github",
        "notion",
        "dropbox",
        "google storage",
        "google one",
        "adobe",
        "microsoft",
        "audible",
        "patreon",
        "substack",
        "medium",
        "linkedin",
        "cursor",
        "midjourney",
    ];
    VENDORS.iter().copied().find(|v| normalized.contains(v))
}

/// Telecom / utility vendors whose recurring charges are *bills* (regular,
/// sometimes larger, sometimes variable within a band).
pub fn bill_vendor_hint(normalized: &str) -> Option<&'static str> {
    const VENDORS: &[&str] = &[
        "freedom mobile",
        "virgin plus",
        "virgin mobile",
        "rogers",
        "bell",
        "telus",
        "fido",
        "koodo",
        "chatr",
        "public mobile",
        "shaw",
        "hydro",
        "fortis",
        "enbridge",
        "insurance",
        "wireless",
        "internet",
    ];
    VENDORS.iter().copied().find(|v| normalized.contains(v))
}

/// Generic "plan / billing" words that describe HOW a vendor charges, not WHICH
/// vendor it is. Stripped when building a grouping key so "<vendor>",
/// "<vendor> subscription", and "<vendor> membership fee" collapse to one key.
const PLAN_WORDS: &[&str] = &[
    "subscription",
    "subscr",
    "membership",
    "installment",
    "instalment",
    "fee",
    "plan",
    "monthly",
    "annual",
    "yearly",
    "recurring",
    "renewal",
    "autopay",
    "dues",
];

/// Parent-company ↔ product-brand aliases: descriptors that bill the same vendor
/// under different names (a company and its product) and must group as one
/// series. This is reference data — extend it as real multi-name vendors are
/// found — not per-transaction special-casing.
const VENDOR_ALIASES: &[(&str, &[&str])] = &[
    ("openai", &["openai", "chatgpt"]),
    ("anthropic", &["anthropic", "claude"]),
];

/// Canonical vendor token for a normalized descriptor, collapsing brand/product
/// aliases of the same company. `None` when no known multi-name vendor matches.
pub fn canonical_vendor(normalized: &str) -> Option<&'static str> {
    VENDOR_ALIASES
        .iter()
        .find(|(_, aliases)| aliases.iter().any(|a| normalized.contains(a)))
        .map(|(canon, _)| *canon)
}

/// Counterparty NAME tokens from a transfer/payment descriptor: alphabetic,
/// ≥3 chars, with the bank/product/direction vocabulary removed. Shared by the
/// e-transfer and bank-bill-payment recovery paths in
/// [`canonical_merchant_key`] so both find the payee the same way. Empty when
/// the descriptor carries no name at all (a bare "PRE-AUTHORIZED PAYMENT").
fn counterparty_name(raw: &str) -> String {
    raw.split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && t.chars().all(|c| c.is_alphabetic()))
        .map(|t| t.to_lowercase())
        .filter(|t| !crate::categorize::TRANSFER_STRUCTURAL_TOKENS.contains(&t.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Stable, vendor-canonical grouping key for recurring detection:
/// [`normalize_merchant`] plus company-alias collapsing and plan-word stripping,
/// so every descriptor for the same vendor — statement padding, product brand
/// (`OPENAI *CHATGPT SUBSCR` vs `CHATGPT SUBSCRIPTION` vs `OPENAI`), or a
/// `subscription`/`membership fee` suffix — groups into ONE series instead of
/// splitting one monthly charge into two "quarterly" ones.
pub fn canonical_merchant_key(raw: &str) -> String {
    let norm = normalize_merchant(raw);
    if let Some(canon) = canonical_vendor(&norm) {
        return canon.to_string();
    }
    // A person-to-person transfer normalizes to structural words only
    // ("internet banking e-transfer") because the recipient is truncated away —
    // so EVERY e-transfer would collapse into one series regardless of who it's
    // to, and a recurring rent-by-e-transfer could never be told apart from an
    // unrelated payment. Re-key such descriptors on the counterparty NAME (the
    // alphabetic tokens that aren't transfer vocabulary) so each recipient forms
    // its own series. An uncategorized one is still dismissed as a transfer
    // downstream; only a user-categorized one surfaces as a real recurring cost.
    let lower = raw.to_lowercase();
    if ["e-transfer", "etransfer", "e transfer", "interac"]
        .iter()
        .any(|k| lower.contains(k))
    {
        let name = counterparty_name(raw);
        if !name.is_empty() {
            return format!("e-transfer {name}");
        }
    }
    // A bank bill payment names the channel and the payee together — "ONLINE
    // BANKING BILL PAYMENT HYDRO ONE" — with the payee LAST, so the 3-token
    // normalize truncates it and every payee through the same channel collapses
    // to one key ("online banking bill"). Even when the payee survives (a short
    // "BILL PAYMENT ROGERS"), the key still carries "bill payment", which the
    // recurring classifier reads as a transfer. Re-key on the payee alone.
    //
    // Triggered on "bill pay" specifically — the substring of "BILL PAYMENT" /
    // "BILL PAY" / "ONLINE BILL PAYMENT" — not bare "online banking", which also
    // fronts plain internal transfers that must stay nameless. Every word this
    // phrase can match ("bill", "pay", "payment") is structural vocabulary that
    // `counterparty_name` strips, so the recovered key is clean; an abbreviated
    // "BILL PYMT" is deliberately NOT matched, because "pymt" is not in that
    // vocabulary and would leak into the key. The BARE payee is the whole point:
    // a channel-prefixed key ("bill-payment hydro one") would match
    // `is_payment_like` and flip a real bill back to a Transfer, the false
    // positive #31 removed. An empty recovery (no payee named) falls through
    // unchanged, so a nameless "BILL PAYMENT" is untouched.
    if lower.contains("bill pay") {
        let name = counterparty_name(raw);
        if !name.is_empty() {
            return name;
        }
    }
    let kept: Vec<&str> = norm
        .split_whitespace()
        .filter(|t| !PLAN_WORDS.contains(&t.trim_matches(|c: char| !c.is_alphanumeric())))
        .collect();
    // Never strip a descriptor down to nothing (e.g. "MEMBERSHIP FEE
    // INSTALLMENT" is all plan-words) — keep the normalized form so it still
    // groups with itself.
    if kept.is_empty() {
        norm
    } else {
        kept.join(" ")
    }
}

/// True when a descriptor names a recurring membership / plan / installment fee.
/// Such a charge is a real recurring commitment regardless of amount jitter
/// (e.g. an annual card fee billed monthly whose price steps mid-year), so the
/// recurring classifier rescues it from the amount-stability heuristic. This is
/// vocabulary, not a vendor list.
pub fn is_membership_like(descriptor_lower: &str) -> bool {
    const PATTERNS: &[&str] = &[
        "membership",
        "installment",
        "instalment",
        "subscription",
        "annual fee",
        "member fee",
        " dues",
    ];
    PATTERNS.iter().any(|p| descriptor_lower.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_merges_company_and_product_brand_variants() {
        // The P1-2 split: three descriptors for the same OpenAI subscription.
        let a = canonical_merchant_key("OPENAI *CHATGPT SUBSCR  SAN FRANCISCO");
        let b = canonical_merchant_key("CHATGPT SUBSCRIPTION    SAN FRANCISCO");
        let c = canonical_merchant_key("OPENAI                  SAN FRANCISCO");
        assert_eq!(a, "openai");
        assert_eq!(a, b);
        assert_eq!(a, c);

        // …and three for Anthropic/Claude.
        let d = canonical_merchant_key("ANTHROPIC               SAN FRANCISCO");
        let e = canonical_merchant_key("CLAUDE.AI SUBSCRIPTION  SAN FRANCISCO");
        let f = canonical_merchant_key("ANTHROPIC* CLAUDE SUB   SAN FRANCISCO");
        assert_eq!(d, "anthropic");
        assert_eq!(d, e);
        assert_eq!(d, f);
    }

    #[test]
    fn canonical_key_strips_plan_words_but_keeps_distinct_vendors() {
        // Same gym, "membership"/"subscription" suffix vs bare → one key.
        assert_eq!(
            canonical_merchant_key("GOLDS GYM MEMBERSHIP"),
            canonical_merchant_key("GOLDS GYM"),
        );
        assert_eq!(
            canonical_merchant_key("GOLDS GYM SUBSCRIPTION"),
            canonical_merchant_key("GOLDS GYM"),
        );
        // Different vendors stay distinct.
        assert_ne!(
            canonical_merchant_key("SPOTIFY  STOCKHOLM"),
            canonical_merchant_key("NETFLIX.COM"),
        );
        // An all-plan-word descriptor is not stripped to nothing.
        assert_eq!(
            canonical_merchant_key("MEMBERSHIP FEE INSTALLMENT"),
            canonical_merchant_key("MEMBERSHIP FEE INSTALLMENT"),
        );
        assert!(!canonical_merchant_key("MEMBERSHIP FEE INSTALLMENT").is_empty());
    }

    #[test]
    fn bank_bill_payments_key_on_the_payee_not_the_channel() {
        // The bug: the payee sits after the channel words, past the 3-token
        // cutoff, so every payee through the same channel collapsed to one key.
        let hydro = canonical_merchant_key("ONLINE BANKING BILL PAYMENT HYDRO ONE");
        let telus = canonical_merchant_key("INTERNET BANKING BILL PAYMENT TELUS");
        let rogers = canonical_merchant_key("BILL PAYMENT ROGERS");

        // Each recovers its own payee — the actual regression guard.
        assert_eq!(hydro, "hydro one");
        assert_eq!(telus, "telus");
        assert_eq!(rogers, "rogers");
        assert_ne!(hydro, telus);
        assert_ne!(hydro, rogers);

        // The same payee through different channels groups together.
        assert_eq!(
            canonical_merchant_key("ONLINE BANKING BILL PAYMENT TELUS"),
            canonical_merchant_key("INTERNET BANKING BILL PAYMENT TELUS"),
        );

        // Crucially, the recovered key carries NO payment vocabulary — a key
        // containing "bill payment" would make the recurring classifier read it
        // as a transfer, undoing #31.
        for k in [&hydro, &telus, &rogers] {
            for banned in ["bill payment", "transfer", "payment received", "e-transfer"] {
                assert!(!k.contains(banned), "recovered key {k:?} leaks payment vocabulary");
            }
        }
    }

    #[test]
    fn real_merchants_without_the_bill_payment_phrase_are_untouched() {
        // The trigger is the bill-payment PHRASE, not any structural word, so a
        // real vendor that merely contains "pay"/"credit"/"money" never fires.
        // Pinned because it is the obvious over-reach if the trigger widens.
        assert_eq!(canonical_merchant_key("CREDIT ONE BANK  LAS VEGAS"), "credit one bank");
        assert_eq!(canonical_merchant_key("MONEY MART  BURNABY"), "money mart");
        assert_eq!(canonical_merchant_key("PAYLESS SHOES  BURNABY"), "payless shoes");
    }

    #[test]
    fn a_bill_payment_with_no_named_payee_falls_through_unchanged() {
        // The phrase matches but there is no counterparty to recover, so the
        // key must not become empty — it falls through to the normal path.
        assert_eq!(canonical_merchant_key("BILL PAYMENT"), "bill payment");
    }

    #[test]
    fn membership_vocabulary_is_recognized() {
        assert!(is_membership_like("membership fee installment"));
        assert!(is_membership_like("claude.ai subscription"));
        assert!(is_membership_like("planet fitness annual fee"));
        assert!(!is_membership_like("mcdonalds west vancouver"));
        assert!(!is_membership_like("uber eats toronto"));
    }

    #[test]
    fn groups_statement_padded_variants_to_same_vendor() {
        let a = normalize_merchant("UBER EATS               HTTPS://HELP.UB");
        let b = normalize_merchant("UBER EATS               TORONTO");
        let c = normalize_merchant("UBER EATS");
        assert_eq!(a, "uber eats");
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn strips_payment_processor_prefixes() {
        assert_eq!(
            normalize_merchant("PAYPAL *STARBUCKSCO     8002352883"),
            normalize_merchant("STARBUCKSCO"),
        );
    }

    #[test]
    fn keeps_distinct_vendors_distinct() {
        assert_ne!(
            normalize_merchant("SPOTIFY                 STOCKHOLM"),
            normalize_merchant("WALMART SUPERCENTER 121 BURNABY"),
        );
        assert_eq!(normalize_merchant("SPOTIFY                 STOCKHOLM"), "spotify");
    }

    #[test]
    fn subscription_and_bill_hints_match_real_vendors() {
        assert!(subscription_vendor_hint(&normalize_merchant("SPOTIFY  STOCKHOLM")).is_some());
        assert!(subscription_vendor_hint(&normalize_merchant("OPENAI *CHATGPT SUBSCR  SAN FRANCISCO")).is_some());
        assert!(subscription_vendor_hint(&normalize_merchant("ANTHROPIC  SAN FRANCISCO")).is_some());
        assert!(subscription_vendor_hint(&normalize_merchant("CLAUDE.AI SUBSCRIPTION  SAN FRANCISCO")).is_some());
        assert!(subscription_vendor_hint(&normalize_merchant("OPENROUTER, INC  NEW YORK")).is_some());
        assert!(bill_vendor_hint(&normalize_merchant("FREEDOM MOBILE  877-946-3184")).is_some());
        // Not subscriptions:
        assert!(subscription_vendor_hint(&normalize_merchant("WALMART SUPERCENTER 121 BURNABY")).is_none());
        assert!(subscription_vendor_hint(&normalize_merchant("MCDONALD'S  WEST VANCOUVER")).is_none());
        assert!(subscription_vendor_hint(&normalize_merchant("EVO CAR SHARE  BURNABY")).is_none());
    }
}
