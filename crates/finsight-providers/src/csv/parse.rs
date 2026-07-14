//! Pure CSV row → ParsedRow. No I/O, no DB.

use crate::csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
use chrono::{DateTime, NaiveDate, Utc};
use finsight_core::models::{NewTransaction, TransactionStatus, TxnActivity};

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedRow {
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub notes: Option<String>,
    pub category_hint: Option<String>,
    pub activity: Option<TxnActivity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    WrongColumnCount { got: usize, expected: usize },
    UnparseableDate(String),
    UnparseableAmount(String),
    MissingRequiredField(&'static str),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WrongColumnCount { got, expected } => {
                write!(f, "expected {expected} columns, got {got}")
            }
            Self::UnparseableDate(s) => write!(f, "could not parse date {s:?}"),
            Self::UnparseableAmount(s) => write!(f, "could not parse amount {s:?}"),
            Self::MissingRequiredField(name) => write!(f, "missing required field {name}"),
        }
    }
}

pub fn parse_row(fields: &[&str], mapping: &CsvImportMapping) -> Result<ParsedRow, ParseError> {
    if fields.len() != mapping.columns.len() {
        return Err(ParseError::WrongColumnCount {
            got: fields.len(),
            expected: mapping.columns.len(),
        });
    }

    let mut date: Option<&str> = None;
    let mut amount: Option<&str> = None;
    let mut debit: Option<&str> = None;
    let mut credit: Option<&str> = None;
    let mut merchant: Option<&str> = None;
    let mut notes: Option<&str> = None;
    let mut category: Option<&str> = None;
    let mut activity_type: Option<&str> = None;
    let mut activity_sub_type: Option<&str> = None;
    let mut symbol: Option<&str> = None;
    let mut security_name: Option<&str> = None;
    let mut quantity_raw: Option<&str> = None;
    let mut unit_price_raw: Option<&str> = None;

    for (idx, role) in mapping.columns.iter().enumerate() {
        let v = fields[idx].trim();
        match role {
            ColumnRole::Date => date = Some(v),
            ColumnRole::Amount => amount = Some(v),
            ColumnRole::Debit => debit = Some(v),
            ColumnRole::Credit => credit = Some(v),
            ColumnRole::Merchant => merchant = Some(v),
            ColumnRole::Notes if !v.is_empty() => notes = Some(v),
            ColumnRole::Category if !v.is_empty() => category = Some(v),
            // Brokerage exports write "-" for "no sub-type"; treat it as empty.
            ColumnRole::ActivityType if !v.is_empty() && v != "-" => activity_type = Some(v),
            ColumnRole::ActivitySubType if !v.is_empty() && v != "-" => {
                activity_sub_type = Some(v)
            }
            ColumnRole::Symbol if !v.is_empty() => symbol = Some(v),
            ColumnRole::SecurityName if !v.is_empty() => security_name = Some(v),
            ColumnRole::Quantity if !v.is_empty() => quantity_raw = Some(v),
            ColumnRole::UnitPrice if !v.is_empty() => unit_price_raw = Some(v),
            ColumnRole::Notes
            | ColumnRole::Category
            | ColumnRole::Skip
            | ColumnRole::ActivityType
            | ColumnRole::ActivitySubType
            | ColumnRole::Symbol
            | ColumnRole::SecurityName
            | ColumnRole::Quantity
            | ColumnRole::UnitPrice => {}
        }
    }

    let date_str = date.ok_or(ParseError::MissingRequiredField("date"))?;
    let posted = parse_date(date_str, &mapping.date_format)?;

    // Amount before merchant: MoneyMovement merchant synthesis needs the sign.
    let amount_cents = match mapping.amount_convention {
        AmountConvention::SplitDebitCredit => {
            let d = debit.unwrap_or("");
            let c = credit.unwrap_or("");
            let d_cents = if d.is_empty() {
                0
            } else {
                parse_amount(d, mapping.decimal_separator)?
            };
            let c_cents = if c.is_empty() {
                0
            } else {
                parse_amount(c, mapping.decimal_separator)?
            };
            c_cents - d_cents
        }
        AmountConvention::NegativeIsOutflow => {
            let a = amount.ok_or(ParseError::MissingRequiredField("amount"))?;
            parse_amount(a, mapping.decimal_separator)?
        }
        AmountConvention::PositiveIsOutflow => {
            let a = amount.ok_or(ParseError::MissingRequiredField("amount"))?;
            -parse_amount(a, mapping.decimal_separator)?
        }
    };

    // A mapped, non-empty Merchant column always wins; otherwise synthesize a
    // DETERMINISTIC merchant from the activity (brokerage exports leave the
    // name column empty on non-trade rows). Determinism matters: re-import
    // dedup matches on merchant + amount + date.
    let merchant = match merchant.filter(|m| !m.is_empty()) {
        Some(m) => m.to_owned(),
        None => match activity_type {
            Some(at) => {
                synthesize_merchant(at, activity_sub_type, symbol, amount_cents)
            }
            None => return Err(ParseError::MissingRequiredField("merchant")),
        },
    };

    let is_trade = activity_type == Some("Trade");
    let quantity = quantity_raw
        .map(|q| parse_number(q, mapping.decimal_separator))
        .transpose()?;
    let unit_price = unit_price_raw
        .map(|p| parse_number(p, mapping.decimal_separator))
        .transpose()?;
    // Hardening: some exports (Wealthsimple) duplicate the cash amount into
    // the quantity column on non-trade rows — meaningless as a unit count, so
    // drop it unless the row also carries a unit price.
    let quantity = if !is_trade && unit_price.is_none() {
        None
    } else {
        quantity
    };

    let activity = activity_type.map(|at| TxnActivity {
        activity_type: at.to_owned(),
        activity_sub_type: activity_sub_type.map(str::to_owned),
        symbol: symbol.map(str::to_owned),
        security_name: security_name.map(str::to_owned),
        quantity,
        unit_price,
    });

    // Trades get a "{qty} @ {price} — {security}" note (verbatim export
    // precision) unless the file has its own Notes column.
    let notes = match notes {
        Some(n) => Some(n.to_owned()),
        None if is_trade => {
            let mut parts = Vec::new();
            if let (Some(q), Some(p)) = (quantity_raw, unit_price_raw) {
                parts.push(format!("{q} @ {p}"));
            }
            if let Some(name) = security_name {
                parts.push(name.to_owned());
            }
            (!parts.is_empty()).then(|| parts.join(" — "))
        }
        None => None,
    };

    Ok(ParsedRow {
        posted_at: posted,
        amount_cents,
        merchant_raw: merchant,
        notes,
        category_hint: category.map(str::to_owned),
        activity,
    })
}

/// Deterministic merchant for activity rows whose merchant column is empty.
/// MoneyMovement wording deliberately says "Transfer" so the existing keyword
/// vocabulary (`categorize::is_transfer`, pairing rule 3) recognizes the row
/// even on code paths that predate activity awareness.
fn synthesize_merchant(
    activity_type: &str,
    sub_type: Option<&str>,
    symbol: Option<&str>,
    amount_cents: i64,
) -> String {
    match activity_type {
        "Trade" => {
            let verb = match sub_type {
                Some("BUY") => "Buy",
                Some("SELL") => "Sell",
                _ => "Trade",
            };
            match symbol {
                Some(s) => format!("{verb} {s}"),
                None => verb.to_string(),
            }
        }
        "Dividend" => match symbol {
            Some(s) => format!("Dividend — {s}"),
            None => "Dividend".to_string(),
        },
        "Interest" => "Interest".to_string(),
        "Tax" => match sub_type {
            Some("NRT") => "Withholding tax (NRT)".to_string(),
            Some(sub) => format!("Tax ({sub})"),
            None => "Tax".to_string(),
        },
        "MoneyMovement" => {
            let direction = if amount_cents >= 0 {
                "Transfer in"
            } else {
                "Transfer out"
            };
            match sub_type {
                Some(sub) => format!("{direction} ({sub})"),
                None => direction.to_string(),
            }
        }
        other => match symbol {
            Some(s) => format!("{other} — {s}"),
            None => other.to_string(),
        },
    }
}

/// Plain numeric field (quantity, unit price): signed decimal at full export
/// precision. Honors the mapping's decimal separator; strips digit grouping.
fn parse_number(s: &str, decimal_separator: char) -> Result<f64, ParseError> {
    let core: String = s
        .trim()
        .chars()
        .filter_map(|c| match c {
            '\u{2212}' => Some('-'),
            ',' if decimal_separator == ',' => Some('.'),
            '.' if decimal_separator == ',' => None,
            ',' if decimal_separator == '.' => None,
            ' ' | '+' => None,
            other => Some(other),
        })
        .collect();
    core.parse()
        .map_err(|_| ParseError::UnparseableAmount(s.to_owned()))
}

fn parse_date(s: &str, fmt: &str) -> Result<DateTime<Utc>, ParseError> {
    NaiveDate::parse_from_str(s, fmt)
        .map(|d| d.and_hms_opt(12, 0, 0).unwrap().and_utc())
        .map_err(|_| ParseError::UnparseableDate(s.to_owned()))
}

fn parse_amount(s: &str, decimal_separator: char) -> Result<i64, ParseError> {
    let trimmed = s.trim();

    // Detect common bank suffixes before stripping punctuation/currency symbols.
    let suffix = trimmed.split_whitespace().last().map(str::to_ascii_uppercase);
    let is_credit = suffix.as_deref() == Some("CR");
    let is_debit = suffix.as_deref() == Some("DR");
    let amount_text = if is_credit || is_debit {
        trimmed
            .rsplit_once(char::is_whitespace)
            .map(|(head, _)| head.trim_end())
            .unwrap_or(trimmed)
    } else {
        trimmed
    };
    let has_trailing_minus = amount_text.ends_with('-');
    let has_parentheses = amount_text.starts_with('(') && amount_text.ends_with(')');

    let core: String = amount_text
        .chars()
        .filter_map(|c| match c {
            '\u{2212}' => Some('-'), // Unicode minus sign → ASCII hyphen
            ',' if decimal_separator == ',' => Some('.'),
            '.' if decimal_separator == ',' => None,
            ',' if decimal_separator == '.' => None,
            ' ' | '$' | '€' | '£' => None,
            '(' | ')' | '+' => None,
            '-' if has_trailing_minus => None, // strip trailing minus; we negate below
            other => Some(other),
        })
        .collect();

    let mut f: f64 = core
        .parse()
        .map_err(|_| ParseError::UnparseableAmount(s.to_owned()))?;

    if has_parentheses || has_trailing_minus {
        f = -f.abs();
    }
    if is_debit {
        f = -f.abs();
    }
    if is_credit {
        f = f.abs();
    }

    Ok((f * 100.0).round() as i64)
}

/// Convenience adapter — sets status to Cleared, category_id to None.
/// `raw_json` is the verbatim source row (headers → values), preserved in
/// `raw_synced_data` exactly like SimpleFin preserves its provider payload,
/// so columns without a mapped role survive the import.
pub fn into_new_transaction(
    parsed: ParsedRow,
    account_id: String,
    raw_json: Option<String>,
) -> NewTransaction {
    NewTransaction {
        account_id,
        posted_at: parsed.posted_at,
        amount_cents: parsed.amount_cents,
        merchant_raw: parsed.merchant_raw,
        category_id: None,
        notes: parsed.notes,
        status: TransactionStatus::Cleared, // CSV imports are always cleared
        imported_id: None,
        source: Some("csv".to_string()),
        raw_synced_data: raw_json,
        pending: false,
        external_tx_id: None,
        external_account_id: None,
        activity: parsed.activity,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(cols: Vec<ColumnRole>, conv: AmountConvention, fmt: &str) -> CsvImportMapping {
        CsvImportMapping {
            skip_header_rows: 0,
            columns: cols,
            date_format: fmt.to_string(),
            amount_convention: conv,
            decimal_separator: '.',
            delimiter: None,
        }
    }

    #[test]
    fn standard_us_negative_outflow() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Safeway", "-8.42"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
        assert_eq!(p.merchant_raw, "Safeway");
    }

    #[test]
    fn amex_positive_outflow_negates() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::PositiveIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Whole Foods", "42.18"], &m).unwrap();
        assert_eq!(p.amount_cents, -4218);
    }

    #[test]
    fn split_debit_credit() {
        let m = map(
            vec![
                ColumnRole::Date,
                ColumnRole::Merchant,
                ColumnRole::Debit,
                ColumnRole::Credit,
            ],
            AmountConvention::SplitDebitCredit,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Safeway", "8.42", ""], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
        let p = parse_row(&["2026-05-15", "Payroll", "", "2200.00"], &m).unwrap();
        assert_eq!(p.amount_cents, 220_000);
    }

    #[test]
    fn mmddyyyy_date_format() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%m/%d/%Y",
        );
        let p = parse_row(&["5/19/2026", "Safeway", "-8.42"], &m).unwrap();
        assert_eq!(p.posted_at.naive_utc().date().to_string(), "2026-05-19");
    }

    #[test]
    fn unicode_minus_sign_accepted() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Safeway", "\u{2212}8.42"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
    }

    #[test]
    fn german_comma_decimal() {
        let mut m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%d.%m.%Y",
        );
        m.decimal_separator = ',';
        let p = parse_row(&["19.05.2026", "REWE", "-12,34"], &m).unwrap();
        assert_eq!(p.amount_cents, -1234);
    }

    #[test]
    fn quoted_field_with_comma_does_not_break() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Smith, Jones & Co.", "-8.42"], &m).unwrap();
        assert_eq!(p.merchant_raw, "Smith, Jones & Co.");
    }

    #[test]
    fn missing_merchant_field_errors() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let err = parse_row(&["2026-05-19", "", "-8.42"], &m).unwrap_err();
        assert!(matches!(err, ParseError::MissingRequiredField("merchant")));
    }

    #[test]
    fn unparseable_date_errors() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let err = parse_row(&["not a date", "Safeway", "-8.42"], &m).unwrap_err();
        assert!(matches!(err, ParseError::UnparseableDate(_)));
    }

    #[test]
    fn wrong_column_count_errors() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let err = parse_row(&["2026-05-19", "Safeway"], &m).unwrap_err();
        assert!(matches!(
            err,
            ParseError::WrongColumnCount {
                got: 2,
                expected: 3
            }
        ));
    }

    #[test]
    fn parentheses_negative_amount() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Refund", "(8.42)"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
    }

    #[test]
    fn trailing_minus_sign() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Refund", "8.42-"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
    }

    #[test]
    fn leading_plus_sign() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Store", "+8.42"], &m).unwrap();
        assert_eq!(p.amount_cents, 842);
    }

    #[test]
    fn cr_dr_suffixes() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Store", "8.42 DR"], &m).unwrap();
        assert_eq!(p.amount_cents, -842);
        let p = parse_row(&["2026-05-19", "Refund", "8.42 CR"], &m).unwrap();
        assert_eq!(p.amount_cents, 842);
        let p = parse_row(&["2026-05-19", "Refund", "8.42 cr"], &m).unwrap();
        assert_eq!(p.amount_cents, 842);
    }

    /// Wealthsimple TFSA export column layout (14 columns):
    /// transaction_date, settlement_date, account_id, account_type,
    /// activity_type, activity_sub_type, direction, symbol, name, currency,
    /// quantity, unit_price, commission, net_cash_amount
    fn wealthsimple_map() -> CsvImportMapping {
        map(
            vec![
                ColumnRole::Date,
                ColumnRole::Skip, // settlement_date
                ColumnRole::Skip, // account_id
                ColumnRole::Skip, // account_type
                ColumnRole::ActivityType,
                ColumnRole::ActivitySubType,
                ColumnRole::Skip, // direction
                ColumnRole::Symbol,
                ColumnRole::SecurityName,
                ColumnRole::Skip, // currency
                ColumnRole::Quantity,
                ColumnRole::UnitPrice,
                ColumnRole::Skip, // commission
                ColumnRole::Amount,
            ],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        )
    }

    #[test]
    fn trade_buy_synthesizes_merchant_note_and_activity() {
        let m = wealthsimple_map();
        let p = parse_row(
            &[
                "2025-01-01",
                "2025-01-01",
                "WS0000000CAD",
                "TFSA",
                "Trade",
                "BUY",
                "LONG",
                "ACME",
                "Acme Corp",
                "CAD",
                "8.1234",
                "15.0876",
                "0",
                "-122.6",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Buy ACME");
        assert_eq!(p.amount_cents, -12_260);
        assert_eq!(p.notes.as_deref(), Some("8.1234 @ 15.0876 — Acme Corp"));
        let a = p.activity.unwrap();
        assert_eq!(a.activity_type, "Trade");
        assert_eq!(a.activity_sub_type.as_deref(), Some("BUY"));
        assert_eq!(a.symbol.as_deref(), Some("ACME"));
        assert_eq!(a.quantity, Some(8.1234));
        assert_eq!(a.unit_price, Some(15.0876));
    }

    #[test]
    fn trade_sell_keeps_signed_negative_quantity() {
        let m = wealthsimple_map();
        let p = parse_row(
            &[
                "2025-02-01",
                "2025-02-01",
                "WS0000000CAD",
                "TFSA",
                "Trade",
                "SELL",
                "LONG",
                "GLOBEX",
                "Globex Corp",
                "CAD",
                "-9.1122",
                "61.2233445566",
                "0",
                "409.93",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Sell GLOBEX");
        assert_eq!(p.amount_cents, 40_993);
        assert_eq!(p.activity.unwrap().quantity, Some(-9.1122));
    }

    #[test]
    fn dividend_interest_tax_moneymovement_synthesis() {
        let m = wealthsimple_map();
        // Dividend: sub_type "-" means none; quantity duplicates cash → dropped.
        let p = parse_row(
            &[
                "2025-05-01", "", "WS0000000CAD", "TFSA", "Dividend", "-", "", "INITECH",
                "Initech Inc", "CAD", "0.03", "", "", "0.03",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Dividend — INITECH");
        let a = p.activity.unwrap();
        assert_eq!(a.activity_sub_type, None);
        assert_eq!(a.quantity, None, "non-trade quantity echo must be dropped");

        let p = parse_row(
            &[
                "2025-02-10", "", "WS0000000CAD", "TFSA", "Interest", "-", "", "", "", "CAD",
                "0.01", "", "", "0.01",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Interest");

        let p = parse_row(
            &[
                "2025-05-15", "", "WS0000000CAD", "TFSA", "Tax", "NRT", "", "", "", "CAD",
                "-0.29", "", "", "-0.29",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Withholding tax (NRT)");

        // MoneyMovement direction comes from the cash sign.
        let p = parse_row(
            &[
                "2025-01-05", "", "WS0000000CAD", "TFSA", "MoneyMovement", "EFT", "", "", "",
                "CAD", "200", "", "", "200",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Transfer in (EFT)");
        let p = parse_row(
            &[
                "2025-06-01", "", "WS0000000CAD", "TFSA", "MoneyMovement", "EFT", "", "", "",
                "CAD", "-75", "", "", "-75",
            ],
            &m,
        )
        .unwrap();
        assert_eq!(p.merchant_raw, "Transfer out (EFT)");
    }

    #[test]
    fn mapped_merchant_wins_over_synthesis() {
        let m = map(
            vec![
                ColumnRole::Date,
                ColumnRole::ActivityType,
                ColumnRole::Merchant,
                ColumnRole::Amount,
            ],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let p = parse_row(&["2026-05-19", "Dividend", "Custom Name", "1.00"], &m).unwrap();
        assert_eq!(p.merchant_raw, "Custom Name");
        // Empty merchant column falls back to synthesis instead of erroring.
        let p = parse_row(&["2026-05-19", "Dividend", "", "1.00"], &m).unwrap();
        assert_eq!(p.merchant_raw, "Dividend");
    }

    #[test]
    fn empty_merchant_without_activity_still_errors() {
        let m = map(
            vec![ColumnRole::Date, ColumnRole::Merchant, ColumnRole::Amount],
            AmountConvention::NegativeIsOutflow,
            "%Y-%m-%d",
        );
        let err = parse_row(&["2026-05-19", "", "-8.42"], &m).unwrap_err();
        assert!(matches!(err, ParseError::MissingRequiredField("merchant")));
    }

    #[test]
    fn into_new_transaction_carries_activity_and_raw_json() {
        let m = wealthsimple_map();
        let p = parse_row(
            &[
                "2025-01-01", "2025-01-01", "WS0000000CAD", "TFSA", "Trade", "BUY", "LONG",
                "ACME", "Acme Corp", "CAD", "8.1234", "15.0876", "0", "-122.6",
            ],
            &m,
        )
        .unwrap();
        let tx = into_new_transaction(p, "acct-1".into(), Some(r#"{"commission":"0"}"#.into()));
        assert!(tx.activity.is_some());
        assert_eq!(tx.raw_synced_data.as_deref(), Some(r#"{"commission":"0"}"#));
    }
}
