/// Single source for balance snapshot source precedence.
///
/// Adding a new sync source (e.g. Plaid) is 1 enum variant + 1 edit here.
/// Every consumer that needs "latest known balance per account" orders by
/// `latest_balance_subquery(alias)` or `balance_snapshot_order(alias, direction)`.
///
/// The source ranking is: `simplefin` (bank-reported) > `ledger_recomputed` (local
/// recompute) > anything else (manual/seed/derived). This matches the spec's
/// `ORDER BY CASE alias.source WHEN 'simplefin' THEN 1 WHEN 'ledger_recomputed' THEN 2 ELSE 3 END`.
///
/// For backward compatibility with existing data that still carries `derived`/`seed`,
/// `balance_snapshot_order` preserves the legacy tiering (`simplefin` 0, else 1,
/// `derived` 2, `seed` 3) but is still single-sourced here so a `rg CASE.*source`
/// guard can enforce no duplication outside this file. New code should prefer
/// `latest_balance_subquery`.
pub fn latest_balance_subquery(alias: &str) -> String {
    let a = alias.trim().trim_end_matches('.');
    if a.is_empty() {
        "ORDER BY CASE source WHEN 'simplefin' THEN 1 WHEN 'ledger_recomputed' THEN 2 ELSE 3 END".to_string()
    } else {
        format!("ORDER BY CASE {a}.source WHEN 'simplefin' THEN 1 WHEN 'ledger_recomputed' THEN 2 ELSE 3 END")
    }
}

/// Legacy date-plus-source ordering used by existing queries:
///
/// `ORDER BY {alias}as_of_date {direction}, CASE {alias}source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END`
///
/// Kept single-sourced in this file so the `CASE.*source` guard stays green.
/// New queries that only need source precedence should use `latest_balance_subquery`.
pub fn balance_snapshot_order(alias: &str, direction: &str) -> String {
    // Keep CASE literal here (single source) — metrics.rs and all repos delegate here.
    format!(
        "{alias}as_of_date {direction}, CASE {alias}source WHEN 'simplefin' THEN 0 WHEN 'derived' THEN 2 WHEN 'seed' THEN 3 ELSE 1 END"
    )
}
