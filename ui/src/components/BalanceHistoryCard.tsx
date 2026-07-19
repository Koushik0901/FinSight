import { useMemo, useState } from "react";
import type { AccountSummary } from "../api/client";
import { useAccountBalanceTimeline } from "../api/hooks/accounts";
import { money } from "../utils/format";
import NetWorthChart from "./NetWorthChart";

const RANGES = [
  { key: "3m", label: "3M", days: 90 },
  { key: "1y", label: "1Y", days: 365 },
  { key: "all", label: "All", days: null },
] as const;

type RangeKey = (typeof RANGES)[number]["key"];

function isoDaysAgo(days: number) {
  const d = new Date();
  d.setDate(d.getDate() - days);
  // Format from LOCAL fields. `toISOString()` reports the UTC calendar date, so
  // west of Greenwich it lands a day late — the same trap `prettyDate` guards
  // against, just in the other direction.
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

function prettyDate(iso: string) {
  // Parse as local, not UTC — `new Date("2024-05-01")` is midnight UTC and
  // renders as the previous day west of Greenwich.
  const [y, m, d] = iso.split("-").map(Number);
  if (!y || !m || !d) return iso;
  return new Date(y, m - 1, d).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

/**
 * An account's balance over time, reconstructed from its transactions, with the
 * high and low points of the visible range.
 *
 * Renders nothing when the balance can't be honestly derived — an investment
 * account holds market value rather than summed cash flow, and a bank-linked
 * account's recorded readings are the source of truth. Both cases already have
 * better surfaces elsewhere, so a placeholder here would just be noise.
 */
export default function BalanceHistoryCard({ account }: { account: AccountSummary }) {
  const [range, setRange] = useState<RangeKey>("1y");
  const since = useMemo(() => {
    const days = RANGES.find((r) => r.key === range)?.days ?? null;
    return days === null ? null : isoDaysAgo(days);
  }, [range]);

  const { data: timeline, isError } = useAccountBalanceTimeline(account.id, since);

  // Hide the card outright only when the balance can't be honestly derived —
  // a verdict that doesn't change with the range, so it never yanks the chrome
  // out from under a click. A range whose data is still loading keeps the header
  // and chips mounted and swaps only the body; otherwise the buttons the user
  // just pressed would vanish and reappear on every uncached range.
  if (isError || (timeline && !timeline.reconstructable)) return null;

  const points = timeline?.points ?? [];
  const peak = timeline?.peak;
  const trough = timeline?.trough;
  // The reconstruction's SHAPE is always right, so the dates hold regardless.
  // Only the LEVEL depends on the opening figure being real.
  const amountsUnanchored = timeline?.anchor === "assumedZero";

  const chartPoints = points.map((p) => ({ date: p.date, totalCents: p.balanceCents }));
  const rangeLabel = RANGES.find((r) => r.key === range)?.label ?? "";

  return (
    <div className="card" style={{ marginTop: 14, padding: 16 }}>
      <div className="row" style={{ justifyContent: "space-between", alignItems: "flex-start", marginBottom: 12 }}>
        <div>
          <div className="h3">Balance history</div>
          <div className="muted" style={{ fontSize: 13, marginTop: 4 }}>
            Rebuilt from this account&rsquo;s transactions, so it finds the real high point &mdash; not
            just the highest day that happened to get recorded.
          </div>
        </div>
        <div className="row row-sm" role="group" aria-label="Balance history range">
          {RANGES.map((r) => (
            <button
              key={r.key}
              type="button"
              className={`chip${range === r.key ? " accent" : ""}`}
              aria-pressed={range === r.key}
              onClick={() => setRange(r.key)}
            >
              {r.label}
            </button>
          ))}
        </div>
      </div>

      {!timeline ? (
        // Distinct from the empty case below: claiming "not enough activity"
        // while the fetch is still in flight would assert something unknown.
        <div className="stub">Rebuilding this account&rsquo;s balance history&hellip;</div>
      ) : points.length < 2 ? (
        <div className="stub">
          Not enough activity in this range to draw a curve. Import more history, or widen the range.
        </div>
      ) : (
        <>
          <NetWorthChart points={chartPoints} embed subject="Balance" rangeLabel={rangeLabel} />

          <div className="row" style={{ gap: 24, marginTop: 12, flexWrap: "wrap" }}>
            {peak && (
              <div className="stat">
                <div className="eyebrow">Highest</div>
                <div className="num money">{money(peak.balanceCents)}</div>
                <div className="muted" style={{ fontSize: 12 }}>{prettyDate(peak.date)}</div>
              </div>
            )}
            {trough && (
              <div className="stat">
                <div className="eyebrow">Lowest</div>
                <div className="num money">{money(trough.balanceCents)}</div>
                <div className="muted" style={{ fontSize: 12 }}>{prettyDate(trough.date)}</div>
              </div>
            )}
          </div>
        </>
      )}

      {amountsUnanchored && (
        <div className="muted" style={{ fontSize: 12, marginTop: 12 }}>
          <strong>Dates are exact; amounts aren&rsquo;t.</strong> This account&rsquo;s history was
          imported behind a zero opening balance, so every figure above is off by the same unknown
          amount. Set this account&rsquo;s current balance to anchor them.
        </div>
      )}
      {timeline?.earliestTxnDate && (
        <div className="muted" style={{ fontSize: 12, marginTop: 8 }}>
          History starts {prettyDate(timeline.earliestTxnDate)} &mdash; anything before that
          isn&rsquo;t imported.
        </div>
      )}
    </div>
  );
}
