//! Positions and portfolio summary for investment accounts, derived at read
//! time from the transactions ledger (V048 activity columns) — never
//! materialized. Deriving keeps re-imports idempotent by construction and the
//! numbers always consistent with what the user sees in the transaction list.
//!
//! Valuation caveat: this is a LOCAL app with no market data. A position's
//! value uses the unit price of its most recent trade — exact at import time,
//! stale afterwards. Consumers must present it as an estimate, and the account
//! balance is only ever updated through an explicit user action.

use crate::error::CoreResult;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use specta::Type;

/// One open position in an investment account, aggregated from Trade rows.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    pub name: Option<String>,
    /// Net units held: SUM(quantity) over all trades (SELL rows are negative).
    pub quantity: f64,
    /// Unit price of the most recent trade in this symbol (dollars).
    pub last_price: Option<f64>,
    /// Date of that most recent trade (RFC3339).
    pub last_trade_at: Option<String>,
    /// quantity × last_price, rounded to cents. None when no price is known.
    pub market_value_cents: Option<i64>,
    /// Net cash put into this symbol: SUM(−amount) over its trades. A closed
    /// round trip leaves the realized P&L here as a negative (profit) or
    /// positive (loss) residue.
    pub invested_cents: i64,
}

/// Ledger-derived portfolio summary for one investment account.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InvestmentSummary {
    /// Cash in the account: opening (seed) balance + every ledger row. Trades,
    /// contributions, dividends, interest, and tax are all cash movements.
    pub cash_cents: i64,
    /// Σ market value of open positions at their last trade price.
    pub positions_value_cents: i64,
    /// cash + positions value — the "portfolio estimate".
    pub portfolio_estimate_cents: i64,
    /// All-time dividend income (activity_type = 'Dividend').
    pub dividend_income_cents: i64,
    /// All-time interest income (activity_type = 'Interest').
    pub interest_income_cents: i64,
    /// All-time withholding tax, as a positive magnitude (rows are negative).
    pub withholding_tax_cents: i64,
    pub open_positions: i64,
    /// True when any symbol nets below zero — a SELL without its earlier BUYs
    /// (partial-history import). The estimate is unreliable; warn, don't hide.
    pub has_negative_quantity: bool,
}

/// Treat |quantity| below this as a closed position (f64 dust from summing
/// fractional share lots).
const CLOSED_EPSILON: f64 = 1e-9;

/// Open positions for an account, aggregated from its Trade rows,
/// alphabetical by symbol. Closed positions (net quantity ≈ 0) are omitted.
pub fn positions_for_account(conn: &Connection, account_id: &str) -> CoreResult<Vec<Position>> {
    let mut stmt = conn.prepare(
        "SELECT t.symbol, SUM(t.quantity), SUM(-t.amount_cents), \
                (SELECT p.unit_price FROM transactions p \
                  WHERE p.account_id = t.account_id AND p.activity_type = 'Trade' \
                    AND p.symbol = t.symbol AND p.unit_price IS NOT NULL \
                  ORDER BY p.posted_at DESC, p.created_at DESC LIMIT 1), \
                (SELECT p.posted_at FROM transactions p \
                  WHERE p.account_id = t.account_id AND p.activity_type = 'Trade' \
                    AND p.symbol = t.symbol AND p.unit_price IS NOT NULL \
                  ORDER BY p.posted_at DESC, p.created_at DESC LIMIT 1), \
                (SELECT p.security_name FROM transactions p \
                  WHERE p.account_id = t.account_id AND p.activity_type = 'Trade' \
                    AND p.symbol = t.symbol AND p.security_name IS NOT NULL \
                  ORDER BY p.posted_at DESC, p.created_at DESC LIMIT 1) \
         FROM transactions t \
         WHERE t.account_id = ?1 AND t.activity_type = 'Trade' AND t.symbol IS NOT NULL \
         GROUP BY t.symbol \
         ORDER BY t.symbol",
    )?;
    let rows = stmt.query_map(params![account_id], |r| {
        let symbol: String = r.get(0)?;
        let quantity: f64 = r.get::<_, Option<f64>>(1)?.unwrap_or(0.0);
        let invested_cents: i64 = r.get::<_, Option<i64>>(2)?.unwrap_or(0);
        let last_price: Option<f64> = r.get(3)?;
        let last_trade_at: Option<String> = r.get(4)?;
        let name: Option<String> = r.get(5)?;
        Ok((symbol, quantity, invested_cents, last_price, last_trade_at, name))
    })?;

    let mut out = Vec::new();
    for row in rows {
        let (symbol, quantity, invested_cents, last_price, last_trade_at, name) = row?;
        if quantity.abs() < CLOSED_EPSILON {
            continue;
        }
        let market_value_cents = last_price.map(|p| (quantity * p * 100.0).round() as i64);
        out.push(Position {
            symbol,
            name,
            quantity,
            last_price,
            last_trade_at,
            market_value_cents,
            invested_cents,
        });
    }
    Ok(out)
}

/// Ledger-derived summary for one investment account.
pub fn summary_for_account(conn: &Connection, account_id: &str) -> CoreResult<InvestmentSummary> {
    // Opening balance: the seed row written at account creation. Manual
    // balance updates (source='manual') are the user's MARKET VALUE estimate,
    // not cash, so they deliberately don't participate here.
    let opening_cents: i64 = conn
        .query_row(
            "SELECT balance_cents FROM account_balances \
             WHERE account_id = ?1 AND source = 'seed' \
             ORDER BY as_of_date ASC LIMIT 1",
            params![account_id],
            |r| r.get(0),
        )
        .unwrap_or(0);

    let (ledger_cents, dividend_income_cents, interest_income_cents, tax_cents): (
        i64,
        i64,
        i64,
        i64,
    ) = conn.query_row(
        "SELECT COALESCE(SUM(amount_cents), 0), \
                COALESCE(SUM(CASE WHEN activity_type = 'Dividend' THEN amount_cents END), 0), \
                COALESCE(SUM(CASE WHEN activity_type = 'Interest' THEN amount_cents END), 0), \
                COALESCE(SUM(CASE WHEN activity_type = 'Tax' THEN amount_cents END), 0) \
         FROM transactions WHERE account_id = ?1",
        params![account_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;

    let positions = positions_for_account(conn, account_id)?;
    let positions_value_cents: i64 = positions
        .iter()
        .filter_map(|p| p.market_value_cents)
        .sum();
    let has_negative_quantity = positions.iter().any(|p| p.quantity < -CLOSED_EPSILON);

    let cash_cents = opening_cents + ledger_cents;
    Ok(InvestmentSummary {
        cash_cents,
        positions_value_cents,
        portfolio_estimate_cents: cash_cents + positions_value_cents,
        dividend_income_cents,
        interest_income_cents,
        withholding_tax_cents: -tax_cents,
        open_positions: positions.len() as i64,
        has_negative_quantity,
    })
}
