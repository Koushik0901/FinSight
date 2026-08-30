use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// Enable Banking account as returned by POST /sessions and GET /accounts/{uid}/details.
/// Real API shape (see docs/api/reference/#accountresource):
/// {
///   "uid": "07cc67f4-...",
///   "account_id": {"iban": "FI04..."},
///   "currency": "EUR",
///   "name": "My Checking",
///   ...
/// }
/// Tests use simplified stub { "id": "acc-a-1", "name": "...", "currency": "EUR" }.
/// This type deserializes from both.
#[derive(Debug, Clone, Serialize)]
pub struct EnableBankingAccount {
    /// Canonical id — prefers `uid`, then `id`, then `account_id.iban` / `account_id.identification`.
    pub id: String,
    pub name: String,
    pub currency: String,
    pub iban: Option<String>,
    /// Full raw payload for debugging / audit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

impl<'de> Deserialize<'de> for EnableBankingAccount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Value::deserialize(deserializer)?;
        // id extraction
        let id = extract_account_id(&v).unwrap_or_default();
        let name = v
            .get("name")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("details").and_then(|x| x.as_str()))
            .or_else(|| v.get("product").and_then(|x| x.as_str()))
            .unwrap_or("")
            .to_string();
        let currency = v
            .get("currency")
            .and_then(|x| x.as_str())
            .unwrap_or("EUR")
            .to_string();
        let iban = extract_iban(&v);
        Ok(Self {
            id,
            name,
            currency,
            iban,
            raw: Some(v),
        })
    }
}

fn extract_account_id(v: &Value) -> Option<String> {
    if let Some(s) = v.get("uid").and_then(|x| x.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = v.get("id").and_then(|x| x.as_str()) {
        return Some(s.to_string());
    }
    // Enable Banking `account_id` is an object { iban: "..." } in real API;
    // some stubs use plain string.
    if let Some(acc) = v.get("account_id") {
        if let Some(s) = acc.as_str() {
            return Some(s.to_string());
        }
        if let Some(s) = acc.get("iban").and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
        if let Some(s) = acc.get("identification").and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    if let Some(s) = v.get("resourceId").and_then(|x| x.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = v.get("accountId").and_then(|x| x.as_str()) {
        return Some(s.to_string());
    }
    None
}

fn extract_iban(v: &Value) -> Option<String> {
    if let Some(s) = v.get("iban").and_then(|x| x.as_str()) {
        return Some(s.to_string());
    }
    if let Some(acc) = v.get("account_id") {
        if let Some(s) = acc.get("iban").and_then(|x| x.as_str()) {
            return Some(s.to_string());
        }
    }
    None
}

/// Balance as returned by GET /accounts/{uid}/balances
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnableBankingBalance {
    #[serde(default)]
    pub balance_type: Option<String>,
    pub balance_amount: AmountType,
    #[serde(default)]
    pub last_change_date_time: Option<String>,
    #[serde(default)]
    pub reference_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmountType {
    pub amount: String,
    pub currency: String,
}

/// Transaction as returned by GET /accounts/{uid}/transactions
/// Real shape:
/// {
///   "transaction_id": "string",
///   "transaction_amount": {"amount": "1.23", "currency": "EUR"},
///   "booking_date": "2020-01-03",
///   "remittance_information": ["RF...","Gift"],
///   "creditor": {"name": "..."},
///   "debtor": {"name": "..."},
///   "credit_debit_indicator": "CRDT",
///   ...
/// }
/// Stub shape for tests: { "id": "...", "amount": "-12.34", "description": "...", "posted": 1700000000 }
#[derive(Debug, Clone, Serialize)]
pub struct EnableBankingTransaction {
    pub id: String,
    /// Amount as string (e.g. "-12.34"), signed. Positive = credit, negative = debit in EB.
    pub amount: String,
    pub currency: String,
    pub booking_date: Option<String>,
    pub value_date: Option<String>,
    pub description: String,
    pub creditor_name: Option<String>,
    pub debtor_name: Option<String>,
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<Value>,
}

impl<'de> Deserialize<'de> for EnableBankingTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let v = Value::deserialize(deserializer)?;
        // id
        let id = v
            .get("transaction_id")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("transactionId").and_then(|x| x.as_str()))
            .or_else(|| v.get("id").and_then(|x| x.as_str()))
            .or_else(|| v.get("entry_reference").and_then(|x| x.as_str()))
            .unwrap_or("")
            .to_string();
        // amount
        let (amount, currency) = if let Some(amt) = v.get("transaction_amount") {
            let a = amt.get("amount").and_then(|x| x.as_str()).unwrap_or("0");
            let c = amt
                .get("currency")
                .and_then(|x| x.as_str())
                .unwrap_or("EUR");
            // EB's credit_debit_indicator tells sign: CRDT = credit (positive), DBIT = debit (negative)
            // Some ASPSPs already sign the amount; we respect indicator if amount is unsigned.
            let indicator = v
                .get("credit_debit_indicator")
                .and_then(|x| x.as_str())
                .unwrap_or("");
            let signed = if indicator == "DBIT" && !a.starts_with('-') {
                format!("-{}", a)
            } else if indicator == "CRDT" && a.starts_with('-') {
                a.trim_start_matches('-').to_string()
            } else {
                a.to_string()
            };
            (signed, c.to_string())
        } else if let Some(s) = v.get("amount").and_then(|x| x.as_str()) {
            let c = v
                .get("currency")
                .and_then(|x| x.as_str())
                .unwrap_or("EUR")
                .to_string();
            (s.to_string(), c)
        } else {
            ("0".to_string(), "EUR".to_string())
        };
        let booking_date = v
            .get("booking_date")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("bookingDate").and_then(|x| x.as_str()))
            .or_else(|| v.get("value_date").and_then(|x| x.as_str()))
            .map(|s| s.to_string());
        let value_date = v
            .get("value_date")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("valueDate").and_then(|x| x.as_str()))
            .map(|s| s.to_string());
        // description: prefer remittance_information[0], then note, then description, then payee
        let description =
            if let Some(arr) = v.get("remittance_information").and_then(|x| x.as_array()) {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else if let Some(s) = v.get("note").and_then(|x| x.as_str()) {
                s.to_string()
            } else if let Some(s) = v.get("description").and_then(|x| x.as_str()) {
                s.to_string()
            } else if let Some(s) = v
                .get("remittanceInformationUnstructured")
                .and_then(|x| x.as_str())
            {
                s.to_string()
            } else {
                v.get("payee")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string()
            };
        let description = if description.is_empty() {
            // fallback to creditor/debtor name
            v.get("creditor")
                .and_then(|x| x.get("name"))
                .and_then(|x| x.as_str())
                .or_else(|| {
                    v.get("debtor")
                        .and_then(|x| x.get("name"))
                        .and_then(|x| x.as_str())
                })
                .unwrap_or("")
                .to_string()
        } else {
            description
        };
        let creditor_name = v
            .get("creditor")
            .and_then(|x| x.get("name"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        let debtor_name = v
            .get("debtor")
            .and_then(|x| x.get("name"))
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        let status = v
            .get("status")
            .and_then(|x| x.as_str())
            .map(|s| s.to_string());
        Ok(Self {
            id: id.clone(),
            amount,
            currency,
            booking_date,
            value_date,
            description: if description.is_empty() {
                id.clone()
            } else {
                description
            },
            creditor_name,
            debtor_name,
            status,
            raw: Some(v),
        })
    }
}
