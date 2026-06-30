use super::models::SimpleFinAccount;
use finsight_core::models::AccountType;

/// Classify a SimpleFin account into a local AccountType + account group.
/// SimpleFin does not expose a formal account type, so we use keyword heuristics
/// on the account name and connection/institution name (OpenCoffer-style).
pub fn classify_account(
    account: &SimpleFinAccount,
    connection_name: Option<&str>,
) -> (AccountType, &'static str) {
    let name = account.name.to_lowercase();
    let blob = format!("{} {}", connection_name.unwrap_or("").to_lowercase(), name);

    if is_credit_card(&name) || is_credit_card(&blob) {
        return (AccountType::Credit, "credit");
    }
    if is_loan(&blob) {
        return (AccountType::Loan, "loan");
    }
    if is_investment(&blob) {
        return (AccountType::Investment, "investment");
    }
    if is_savings(&name) {
        return (AccountType::Savings, "cash");
    }
    if is_checking(&name) || is_checking(&blob) {
        return (AccountType::Checking, "cash");
    }

    (AccountType::Other, "other")
}

fn is_credit_card(s: &str) -> bool {
    [
        "visa",
        "mastercard",
        "amex",
        "american express",
        "credit card",
        "rewards card",
        "discover",
        "citi ",
        "capital one",
        "chase sapphire",
        "platinum",
        "gold card",
    ]
    .iter()
    .any(|hint| s.contains(hint))
}

fn is_loan(s: &str) -> bool {
    [
        "loan",
        "mortgage",
        "auto loan",
        "student loan",
        "home equity",
        "heloc",
        "personal loan",
        "car loan",
    ]
    .iter()
    .any(|hint| s.contains(hint))
}

fn is_investment(s: &str) -> bool {
    [
        "401k",
        "403b",
        "ira",
        "roth",
        "brokerage",
        "investment",
        "hsa",
        "rsp",
        "rrsp",
        "tfsa",
        "529",
        "fidelity",
        "vanguard",
        "schwab",
        "merrill",
        "etrade",
        "td ameritrade",
    ]
    .iter()
    .any(|hint| s.contains(hint))
}

fn is_savings(s: &str) -> bool {
    ["savings", "save", "money market", "high yield"]
        .iter()
        .any(|hint| s.contains(hint))
}

fn is_checking(s: &str) -> bool {
    ["checking", "chequing", "current account", "debit"]
        .iter()
        .any(|hint| s.contains(hint))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(name: &str) -> SimpleFinAccount {
        SimpleFinAccount {
            id: "1".into(),
            name: name.into(),
            connection_name: None,
            connection_id: None,
            currency: "USD".into(),
            balance: "0.00".into(),
            available_balance: None,
            balance_date: 0,
            transactions: None,
            extra: None,
        }
    }

    #[test]
    fn classify_credit() {
        assert_eq!(
            classify_account(&account("My Visa"), None),
            (AccountType::Credit, "credit")
        );
        assert_eq!(
            classify_account(&account("Rewards Card"), None),
            (AccountType::Credit, "credit")
        );
    }

    #[test]
    fn classify_savings() {
        assert_eq!(
            classify_account(&account("High Yield Savings"), None),
            (AccountType::Savings, "cash")
        );
    }

    #[test]
    fn classify_investment() {
        assert_eq!(
            classify_account(&account("Fidelity Brokerage"), None),
            (AccountType::Investment, "investment")
        );
        assert_eq!(
            classify_account(&account("401k"), None),
            (AccountType::Investment, "investment")
        );
    }

    #[test]
    fn classify_loan() {
        assert_eq!(
            classify_account(&account("Mortgage"), None),
            (AccountType::Loan, "loan")
        );
    }

    #[test]
    fn classify_checking_default() {
        assert_eq!(
            classify_account(&account("Everyday"), None),
            (AccountType::Other, "other")
        );
        assert_eq!(
            classify_account(&account("Primary Checking"), None),
            (AccountType::Checking, "cash")
        );
    }
}
