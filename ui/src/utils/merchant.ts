/**
 * Display-clean a raw bank merchant string. Statement descriptors carry
 * machine noise — column-aligned space runs ("TIM HORTONS #3356       BURNABY")
 * and support-URL tails ("UBER EATS               HTTPS://HELP.UB") — that
 * leaks straight into tables. This trims the noise WITHOUT renaming anything:
 * casing and words stay exactly as the bank wrote them, so the user can always
 * match a row back to their statement. Display-only — never use the result as
 * a key, a rule pattern, or an editable field value.
 */
export function prettyMerchant(raw: string): string {
  const cleaned = raw
    .split(/\s+/)
    // Drop pure-URL tokens (support links, not identity). Domain-style NAMES
    // like "TEMU.COM" stay — only scheme/www prefixes mark a URL token.
    .filter((tok) => !/^(https?:\/\/|www\.)/i.test(tok))
    .join(" ")
    .trim();
  // Never return an empty display name; fall back to the raw string.
  return cleaned || raw.trim();
}
