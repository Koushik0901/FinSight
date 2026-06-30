import type { Account, AccountSummary } from "../api/client";

export function getAccountDisplayName(account: Account | AccountSummary): string {
  return account.nickname || account.official_name || account.name;
}
