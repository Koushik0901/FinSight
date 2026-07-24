import type { ReactNode } from "react";

/**
 * Currency tokens the app itself emits via `money()` / `compactMoney()` and
 * then embeds inside prose — "$18", "$1,800", "$1800", "$3.50", "$35k",
 * "CA$1.2M". Deliberately anchored on the `$` sign (optionally with a 1–3
 * letter currency prefix): we only want to blur money, never bare numbers like
 * dates, day counts or percentages, which must stay readable in privacy mode.
 */
const AMOUNT_TOKEN = /([A-Z]{0,3}\$\s?\d[\d,]*(?:\.\d+)?[kKmM]?)/g;

/**
 * Split an app-generated string so each embedded currency amount is wrapped in
 * a `.blurable` span — the same privacy-mode hook the standalone `.money`
 * figures use. This lets us hide the amount inside a sentence ("NETFLIX.COM
 * (about $18) is due…") without blurring the merchant, date or advice around it.
 *
 * Use this ONLY on app-formatted strings, never on user-authored free text
 * (e.g. a scenario the user named "Buy a car $35k") — blurring a substring of
 * someone's own words is both fragile and semantically wrong.
 *
 * `String.split` with a single capturing group returns alternating
 * [text, amount, text, amount, …], so odd indices are the captured amounts —
 * which sidesteps the `lastIndex` statefulness of testing a `/g` regex per part.
 */
export function blurAmounts(text: string): ReactNode {
  const parts = text.split(AMOUNT_TOKEN);
  if (parts.length === 1) return text;
  return parts.map((part, i) =>
    i % 2 === 1 ? (
      <span key={i} className="blurable">
        {part}
      </span>
    ) : (
      part
    ),
  );
}
