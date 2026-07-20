//! What currencies the user actually holds, derived from their accounts.
//!
//! Every `_cents` aggregate in this crate is a plain `i64`. Nothing in an `i64`
//! records which currency it counts, so summing across accounts denominated
//! differently produces a number that looks exactly like a real one and is
//! meaningless. That was invisible in single-currency sample data and ordinary
//! in the general population: cross-border workers, expats, anyone holding a
//! USD account at a Canadian bank, anyone who kept an account after moving
//! country. Mixed-currency data can arrive on day one through CSV import or a
//! bank feed.
//!
//! **The rule this module exists to enforce: no aggregate is ever a
//! cross-currency sum.** Aggregates are scoped to one currency — the primary —
//! and holdings in every other currency are reported alongside, unconverted and
//! explicitly labelled, so the user sees an honest partial answer instead of a
//! confident wrong total.
//!
//! Deliberately NOT done here, and why:
//!
//! * **No FX conversion.** It needs a network call to a rate provider, which
//!   breaks local-first operation and adds a third party the app does not have.
//!   It also makes historical figures depend on which rate you pick
//!   (transaction-date vs today).
//! * **No user-entered static rates.** They go stale silently — the figure
//!   still looks like a figure and nothing detects the decay. That is the same
//!   class of quietly-wrong number, just slower.
//! * **The currency set is never read from a setting.** A display preference
//!   goes stale exactly like a static rate would: the user picks CAD, later
//!   opens a USD account, and the label keeps saying CAD. Deriving it from the
//!   accounts themselves means there is nothing to maintain and nothing to
//!   drift.

use crate::error::CoreResult;
use rusqlite::Connection;

/// What the `accounts.currency` column holds when nobody set it (the V001
/// schema default). A blank or whitespace-only value is treated as this, since
/// an empty string is indistinguishable in intent from never having been set.
pub const SCHEMA_DEFAULT_CURRENCY: &str = "USD";

/// Fold a currency code from any source — CSV import, bank feed, manual entry —
/// into a comparable key. Trims and upper-cases, so `"cad"`, `" CAD"`, and
/// `"CAD"` are one currency rather than three.
///
/// Codes that are not ISO-4217-shaped are passed through normalized rather than
/// rejected: we cannot know that an unfamiliar code is wrong, and dropping an
/// account because its currency looks unusual would lose real money from the
/// user's net worth. They simply form their own bucket and are never merged
/// with anything else. Display code must not assume the result is renderable as
/// a currency symbol — see `looks_iso4217`.
pub fn normalize_code(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return SCHEMA_DEFAULT_CURRENCY.to_string();
    }
    trimmed.to_ascii_uppercase()
}

/// Whether a normalized code has the shape ISO 4217 defines — exactly three
/// ASCII letters. Currency *formatters* (including JavaScript's
/// `Intl.NumberFormat`, which throws on anything else) require this, so callers
/// that render symbols must check before formatting and fall back to showing
/// the raw code.
pub fn looks_iso4217(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_alphabetic())
}

/// One currency the user holds, with enough detail to headline it or to list it
/// as an unconverted aside.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CurrencyHolding {
    /// Normalized code (see [`normalize_code`]).
    pub code: String,
    /// Non-archived accounts denominated in it.
    pub account_count: i64,
    /// Signed sum of the *known* balances of those accounts. Accounts whose
    /// balance is unconfirmed are excluded here for the same reason
    /// `balance_breakdown` excludes them — a phantom $0 is not a balance.
    pub balance_cents: i64,
}

/// The currency composition of the user's holdings.
///
/// Ordered most-held first: by account count, then by absolute known balance,
/// then by code. The last two are tie-breakers that exist so the primary
/// currency is *deterministic* — a headline figure that silently swapped
/// currency between two page loads because a `HashMap` reordered would be worse
/// than the bug this module fixes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CurrencyProfile {
    pub holdings: Vec<CurrencyHolding>,
}

impl CurrencyProfile {
    /// The currency aggregates are denominated in. `None` only when there are
    /// no accounts at all, in which case there is nothing to denominate.
    pub fn primary(&self) -> Option<&str> {
        self.holdings.first().map(|h| h.code.as_str())
    }

    /// True when holdings span more than one currency, i.e. when any total that
    /// ignored currency would be a cross-currency sum.
    pub fn is_mixed(&self) -> bool {
        self.holdings.len() > 1
    }

    /// Everything except the primary — held, never converted, never added in.
    pub fn unconverted(&self) -> &[CurrencyHolding] {
        self.holdings.get(1..).unwrap_or(&[])
    }
}

/// Derive the currency profile from live account data.
///
/// Scoped to non-archived accounts, matching `accounts::list_summaries`: an
/// account the user closed should not keep a second currency alive and force
/// every total to be caveated forever.
pub fn currency_profile(conn: &Connection) -> CoreResult<CurrencyProfile> {
    // Balance selection mirrors `accounts::list_summaries` — latest as-of date,
    // ties broken by source precedence — and the `balance_known` rule mirrors
    // it too, so a currency's subtotal here can never disagree with the same
    // accounts' contribution to `balance_breakdown`.
    //
    // Cached because scoping asks for the profile once per aggregate, and a
    // single metrics request runs several — re-preparing this each time is pure
    // overhead on the app's hottest read path.
    let mut stmt = conn.prepare_cached(
        "SELECT a.currency, \
                COALESCE((SELECT b.balance_cents FROM account_balances b \
                          WHERE b.account_id = a.id \
                          ORDER BY b.as_of_date DESC, \
                            CASE b.source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 \
                                          WHEN 'seed' THEN 3 ELSE 1 END \
                          LIMIT 1), 0) AS balance, \
                CASE \
                  WHEN EXISTS (SELECT 1 FROM account_balances b \
                               WHERE b.account_id = a.id AND b.source <> 'seed') THEN 1 \
                  WHEN NOT EXISTS (SELECT 1 FROM transactions t WHERE t.account_id = a.id) THEN 1 \
                  ELSE 0 \
                END AS balance_known \
         FROM accounts a \
         WHERE a.archived_at IS NULL",
    )?;

    let mut holdings: Vec<CurrencyHolding> = Vec::new();
    for row in stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, i64>(2)? != 0,
        ))
    })? {
        let (raw, balance, known) = row?;
        let code = normalize_code(&raw);
        match holdings.iter_mut().find(|h| h.code == code) {
            Some(h) => {
                h.account_count += 1;
                if known {
                    h.balance_cents += balance;
                }
            }
            None => holdings.push(CurrencyHolding {
                code,
                account_count: 1,
                balance_cents: if known { balance } else { 0 },
            }),
        }
    }

    holdings.sort_by(|a, b| {
        b.account_count
            .cmp(&a.account_count)
            .then_with(|| b.balance_cents.abs().cmp(&a.balance_cents.abs()))
            .then_with(|| a.code.cmp(&b.code))
    });

    Ok(CurrencyProfile { holdings })
}

/// Quote a value as a SQLite string literal.
///
/// SQLite string literals have exactly one escape: a single quote is written
/// twice. There are no backslash escapes, so doubling quotes is complete rather
/// than merely best-effort, and the result cannot terminate the literal early,
/// start a comment, or introduce a second statement.
fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// SQL predicate restricting a transaction (on the given `transactions` alias)
/// to accounts denominated in `code`.
///
/// The code is embedded rather than bound because these predicates compose into
/// `format!`-built SQL shared by several call sites, and threading an extra
/// positional parameter through all of them would renumber every existing bind.
/// It is quoted through [`sql_string_literal`], which is airtight for arbitrary
/// text — so scoping works for ANY currency code, not only ISO-shaped ones.
///
/// That matters for consistency, not just coverage: `balance_breakdown` filters
/// balances with a plain Rust string comparison that works for any code, so if
/// this declined to scope unusual codes, one half of a metrics payload would be
/// narrowed and the other half would not, under a single shared caveat.
pub fn same_currency_txn_predicate(alias: &str, code: &str) -> String {
    let literal = sql_string_literal(&normalize_code(code));
    format!(
        "{alias}.account_id IN (SELECT id FROM accounts WHERE UPPER(TRIM(currency)) = {literal})"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_folds_case_and_whitespace_but_keeps_unfamiliar_codes() {
        assert_eq!(normalize_code("cad"), "CAD");
        assert_eq!(normalize_code(" CAD "), "CAD");
        assert_eq!(normalize_code("CAD"), "CAD");
        // Blank behaves as the column default would if it had never been set.
        assert_eq!(normalize_code(""), SCHEMA_DEFAULT_CURRENCY);
        assert_eq!(normalize_code("   "), SCHEMA_DEFAULT_CURRENCY);
        // An unfamiliar code is kept, not dropped — we cannot know it is wrong,
        // and dropping it would silently remove money from net worth.
        assert_eq!(normalize_code("bitcoin"), "BITCOIN");
    }

    #[test]
    fn iso4217_shape_gates_symbol_formatting() {
        assert!(looks_iso4217("USD"));
        assert!(looks_iso4217("CAD"));
        assert!(!looks_iso4217("US"));
        assert!(!looks_iso4217("USDD"));
        assert!(!looks_iso4217("US1"));
        assert!(!looks_iso4217(""));
    }

    #[test]
    fn currency_predicate_neutralizes_quotes_instead_of_trusting_the_code() {
        let p = same_currency_txn_predicate("t", "cad");
        assert!(p.contains("'CAD'"), "code is normalized into the predicate");

        // An injection attempt is quoted, not executed: the embedded quote is
        // doubled, so the literal never terminates early and no second
        // statement or comment can start.
        let hostile = same_currency_txn_predicate("t", "US' OR '1'='1");
        assert!(
            hostile.contains("'US'' OR ''1''=''1'"),
            "quotes are doubled: {hostile}"
        );
        assert!(
            !hostile.contains("' OR '1'='1'"),
            "no unescaped injection payload survives: {hostile}"
        );

        // A code we do not recognise still scopes, so balances and flows narrow
        // by the same rule rather than one half silently staying wide.
        assert!(same_currency_txn_predicate("t", "BITCOIN").contains("'BITCOIN'"));
    }

    #[test]
    fn sql_literal_quoting_is_complete_for_arbitrary_text() {
        assert_eq!(sql_string_literal("USD"), "'USD'");
        assert_eq!(sql_string_literal("O'Brien"), "'O''Brien'");
        assert_eq!(sql_string_literal("''"), "''''''");
        // Backslash is NOT an escape character in SQLite string literals, so it
        // needs no special handling and must pass through untouched.
        assert_eq!(sql_string_literal("a\\'b"), "'a\\''b'");
    }

    #[test]
    fn empty_profile_has_no_primary_and_is_not_mixed() {
        let p = CurrencyProfile::default();
        assert_eq!(p.primary(), None);
        assert!(!p.is_mixed());
        assert!(p.unconverted().is_empty());
    }

    #[test]
    fn ordering_is_deterministic_when_account_counts_tie() {
        // One account each: the tie must break on balance, then on code —
        // never on row order, or the headline currency would flip between
        // page loads.
        let profile = CurrencyProfile {
            holdings: vec![
                CurrencyHolding {
                    code: "AAA".into(),
                    account_count: 1,
                    balance_cents: 100,
                },
                CurrencyHolding {
                    code: "ZZZ".into(),
                    account_count: 1,
                    balance_cents: 900,
                },
            ],
        };
        let mut sorted = profile.holdings.clone();
        sorted.sort_by(|a, b| {
            b.account_count
                .cmp(&a.account_count)
                .then_with(|| b.balance_cents.abs().cmp(&a.balance_cents.abs()))
                .then_with(|| a.code.cmp(&b.code))
        });
        assert_eq!(sorted[0].code, "ZZZ", "larger balance wins the tie");
    }
}
