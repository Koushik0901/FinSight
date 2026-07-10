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
