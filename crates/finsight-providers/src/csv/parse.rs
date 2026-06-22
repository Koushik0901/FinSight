//! Pure CSV row → ParsedRow. No I/O, no DB.

use crate::csv::mapping::{AmountConvention, ColumnRole, CsvImportMapping};
use chrono::{DateTime, NaiveDate, Utc};
use finsight_core::models::{NewTransaction, TransactionStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRow {
    pub posted_at: DateTime<Utc>,
    pub amount_cents: i64,
    pub merchant_raw: String,
    pub notes: Option<String>,
    pub category_hint: Option<String>,
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
            ColumnRole::Notes | ColumnRole::Category | ColumnRole::Skip => {}
        }
    }

    let merchant = merchant
        .ok_or(ParseError::MissingRequiredField("merchant"))?
        .to_owned();
    if merchant.is_empty() {
        return Err(ParseError::MissingRequiredField("merchant"));
    }
    let date_str = date.ok_or(ParseError::MissingRequiredField("date"))?;
    let posted = parse_date(date_str, &mapping.date_format)?;

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

    Ok(ParsedRow {
        posted_at: posted,
        amount_cents,
        merchant_raw: merchant,
        notes: notes.map(str::to_owned),
        category_hint: category.map(str::to_owned),
    })
}

fn parse_date(s: &str, fmt: &str) -> Result<DateTime<Utc>, ParseError> {
    NaiveDate::parse_from_str(s, fmt)
        .map(|d| d.and_hms_opt(12, 0, 0).unwrap().and_utc())
        .map_err(|_| ParseError::UnparseableDate(s.to_owned()))
}

fn parse_amount(s: &str, decimal_separator: char) -> Result<i64, ParseError> {
    let cleaned: String = s
        .chars()
        .filter_map(|c| match c {
            '\u{2212}' => Some('-'), // Unicode minus sign → ASCII hyphen
            ',' if decimal_separator == ',' => Some('.'),
            '.' if decimal_separator == ',' => None,
            ',' if decimal_separator == '.' => None,
            ' ' | '$' | '€' | '£' => None,
            other => Some(other),
        })
        .collect();
    let f: f64 = cleaned
        .parse()
        .map_err(|_| ParseError::UnparseableAmount(s.to_owned()))?;
    Ok((f * 100.0).round() as i64)
}

/// Convenience adapter — sets status to Cleared, category_id to None.
pub fn into_new_transaction(parsed: ParsedRow, account_id: String) -> NewTransaction {
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
}
