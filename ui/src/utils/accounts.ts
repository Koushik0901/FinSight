import type { Account, AccountSummary, AccountType } from "../api/client";

export function getAccountDisplayName(account: Account | AccountSummary): string {
  return account.nickname || account.official_name || account.name;
}

export function getAccountTypeColor(type: AccountType): string {
  switch (type) {
    case "Checking":
      return "var(--c-checking)";
    case "Savings":
      return "var(--c-savings)";
    case "Credit":
      return "var(--c-credit)";
    case "Investment":
      return "var(--c-investment)";
    case "Cash":
      return "var(--c-cash)";
    case "Loan":
      return "var(--c-loan)";
    case "Other":
    default:
      return "var(--c-other)";
  }
}
