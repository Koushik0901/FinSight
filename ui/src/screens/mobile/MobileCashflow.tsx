import { useState } from "react";
import { useCashflowForecast } from "../../api/hooks/cashflow";
import { useFinancialMetrics } from "../../api/hooks/metrics";
import type { CashflowForecast, CashflowEvent } from "../../api/openapiClient";
import { money } from "../../utils/format";
import { blurAmounts } from "../../utils/blurAmounts";
import { MobileStat } from "../../components/mobile/MobileStat";
import { MobileSection, MobileList, MobileListItem } from "../../components/mobile/MobileList";
import { SegmentedControl } from "../../components/mobile/SegmentedControl";

const HORIZONS = [30, 60, 90] as const;
type Horizon = (typeof HORIZONS)[number];

function shortDate(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function toCents(dollars: string): number {
  const n = Number(dollars.replace(/[^0-9.]/g, ""));
  return Number.isFinite(n) ? Math.round(n * 100) : 0;
}

function currencySymbol(currency?: string): string {
  try {
    const fmt = new Intl.NumberFormat(undefined, { style: "currency", currency: currency ?? "USD", currencyDisplay: "narrowSymbol" });
    const part = fmt.formatToParts(1).find((p) => p.type === "currency");
    return part?.value ?? "$";
  } catch {
    return "$";
  }
}

/** Simplified mobile chart: full-width line+area, no tiny legend. */
function ProjectedBalanceChart({
  forecast,
  currency,
}: {
  forecast: CashflowForecast;
  currency?: string;
}) {
  const days = forecast.days;
  if (days.length < 2) return null;
  const W = 900;
  const H = 240;
  const padY = 16;
  const balances = days.map((d) => d.projectedBalanceCents);
  const buffer = forecast.bufferCents;
  const lo = Math.min(...balances, buffer, 0);
  const hi = Math.max(...balances, forecast.startBalanceCents, buffer);
  const range = Math.max(hi - lo, 1);
  const x = (i: number) => (i / (days.length - 1)) * W;
  const y = (c: number) => H - padY - ((c - lo) / range) * (H - 2 * padY);
  const linePath = days
    .map((d, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(d.projectedBalanceCents).toFixed(1)}`)
    .join(" ");
  const areaPath = `${linePath} L${W},${H} L0,${H} Z`;
  const bufferY = y(buffer);
  const lowIdx = Math.max(0, days.findIndex((d) => d.date === forecast.lowestDate));

  return (
    <div style={{ width: "100%", marginTop: 12 }}>
      <svg
        viewBox="0 0 900 240"
        width="100%"
        height={H}
        preserveAspectRatio="none"
        style={{ display: "block", overflow: "visible" }}
        role="img"
        aria-label={`Projected balance over ${forecast.horizonDays} days, lowest ${money(forecast.lowestBalanceCents, currency ? { currency } : undefined)} on ${shortDate(forecast.lowestDate)}`}
      >
        <defs>
          <linearGradient id="cf-fill-mobile" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="var(--accent)" stopOpacity={0.2} />
            <stop offset="1" stopColor="var(--accent)" stopOpacity={0} />
          </linearGradient>
        </defs>
        {lo < 0 && hi > 0 ? (
          <line x1={0} y1={y(0)} x2={W} y2={y(0)} stroke="var(--negative)" strokeWidth={1} opacity={0.5} />
        ) : null}
        <line x1={0} y1={bufferY} x2={W} y2={bufferY} stroke="var(--warning)" strokeWidth={1} strokeDasharray="5 5" opacity={0.75} />
        <path d={areaPath} fill="url(#cf-fill-mobile)" />
        <path d={linePath} fill="none" stroke="var(--accent)" strokeWidth={2} strokeLinejoin="round" strokeLinecap="round" />
        <circle cx={x(lowIdx)} cy={y(forecast.lowestBalanceCents)} r={5} fill="var(--negative)" stroke="var(--bg)" strokeWidth={2} />
      </svg>
      {/* Single-line caption instead of multi-item tiny legend */}
      <div
        style={{
          display: "flex",
          alignItems: "center",
          gap: 6,
          marginTop: 8,
          fontSize: 12,
          color: "var(--ink-mute)",
        }}
      >
        <span style={{ width: 12, height: 3, borderRadius: 999, background: "var(--accent)", display: "inline-block" }} aria-hidden="true" />
        <span>Projected balance</span>
        <span aria-hidden="true" style={{ color: "var(--ink-faint)" }}>
          ·
        </span>
        <span style={{ display: "inline-flex", alignItems: "center", gap: 4 }}>
          <span style={{ width: 8, height: 8, borderRadius: "50%", background: "var(--negative)", display: "inline-block" }} aria-hidden="true" />
          lowest {shortDate(forecast.lowestDate)}
        </span>
      </div>
    </div>
  );
}

function eventDotColor(e: CashflowEvent): string {
  if (e.kind === "income") return "var(--positive)";
  if (e.kind === "hypothetical") return "var(--warning)";
  return "var(--ink-faint)";
}

export default function MobileCashflow() {
  const [horizon, setHorizon] = useState<Horizon>(30);
  const [bufferInput, setBufferInput] = useState("0");
  const [testInput, setTestInput] = useState("");
  const bufferCents = toCents(bufferInput);
  const extraExpenseCents = toCents(testInput);

  const { data: metrics } = useFinancialMetrics();
  const currency = metrics?.currency ?? undefined;
  const cur = currency ? { currency } : undefined;

  const { data: forecast, isLoading, isError } = useCashflowForecast({ horizonDays: horizon, bufferCents, extraExpenseCents });

  const cautions = forecast?.warnings.filter((w) => w.level === "caution") ?? [];
  const infos = forecast?.warnings.filter((w) => w.level === "info") ?? [];

  const horizonOptions: Array<{ value: string; label: string }> = HORIZONS.map((h) => ({ value: String(h), label: `${h}d` }));

  if (isError) {
    return (
      <div style={{ display: "flex", flexDirection: "column", gap: 16, padding: 16, paddingBottom: "calc(24px + env(safe-area-inset-bottom))" }}>
        <div style={{ padding: 16, borderRadius: 16, border: "1px solid var(--line)", background: "var(--surface)" }}>
          <p className="muted" style={{ margin: 0 }}>
            Couldn&rsquo;t build the forecast right now.
          </p>
        </div>
      </div>
    );
  }

  if (isLoading && !forecast) {
    return (
      <div className="stub" style={{ minHeight: "40vh" }}>
        <span className="spinner" aria-hidden="true" /> Projecting your next {horizon} days…
      </div>
    );
  }

  if (!forecast) return null;

  const lastDate = forecast.days[forecast.days.length - 1]?.date ?? forecast.lowestDate;

  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        gap: 16,
        padding: 16,
        paddingBottom: "calc(24px + env(safe-area-inset-bottom))",
        paddingLeft: "max(16px, env(safe-area-inset-left))",
        paddingRight: "max(16px, env(safe-area-inset-right))",
      }}
    >
      {/* Hero: safe-to-spend — prominent */}
      <div className="mobile-stat hero" style={{ padding: 16 }}>
        <span className="mobile-stat-label">Safe to spend now</span>
        <span className="mobile-stat-value lg money" style={{ color: "var(--ink)" }}>
          {money(forecast.safeToSpendCents, cur)}
        </span>
        <span className="mobile-stat-sub" style={{ marginTop: 6, lineHeight: 1.5 }}>
          What you can spend today and still keep{" "}
          {bufferCents > 0 ? `at least your ${money(bufferCents, cur)} buffer` : "a positive balance"} through {shortDate(lastDate)} — after bills, subscriptions, and everyday spending.
          {!forecast.reliable ? " This is a rough estimate — there isn’t much history yet." : null}
        </span>
      </div>

      {/* Horizon — SegmentedControl, thumb-friendly */}
      <SegmentedControl
        options={horizonOptions}
        value={String(horizon)}
        onChange={(v) => setHorizon(Number(v) as Horizon)}
        ariaLabel="Forecast horizon"
        fullWidth
      />

      {/* Thumb-friendly 44px inputs — stacked, not squeezed toolbar */}
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        <label htmlFor="mcf-buffer" style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span style={{ fontSize: 12, fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase", color: "var(--ink-faint)" }}>
            Keep a safety buffer of
          </span>
          <span
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              minHeight: 44,
              padding: "0 12px",
              border: "1px solid var(--line)",
              borderRadius: 12,
              background: "var(--surface)",
            }}
          >
            <span style={{ color: "var(--ink-mute)", fontWeight: 600, fontSize: 15 }}>{currencySymbol(currency)}</span>
            <input
              id="mcf-buffer"
              inputMode="decimal"
              value={bufferInput}
              onChange={(e) => setBufferInput(e.target.value)}
              placeholder="0"
              style={{
                flex: 1,
                border: "none",
                outline: "none",
                background: "transparent",
                fontSize: 16,
                minHeight: 44,
                color: "var(--ink)",
              }}
            />
          </span>
        </label>

        <label htmlFor="mcf-test" style={{ display: "flex", flexDirection: "column", gap: 6 }}>
          <span style={{ fontSize: 12, fontWeight: 600, letterSpacing: "0.06em", textTransform: "uppercase", color: "var(--ink-faint)" }}>
            Test a purchase
          </span>
          <span
            style={{
              display: "flex",
              alignItems: "center",
              gap: 8,
              minHeight: 44,
              padding: "0 12px",
              border: "1px solid var(--line)",
              borderRadius: 12,
              background: "var(--surface)",
            }}
          >
            <span style={{ color: "var(--ink-mute)", fontWeight: 600, fontSize: 15 }}>{currencySymbol(currency)}</span>
            <input
              id="mcf-test"
              inputMode="decimal"
              placeholder="0"
              value={testInput}
              onChange={(e) => setTestInput(e.target.value)}
              style={{
                flex: 1,
                border: "none",
                outline: "none",
                background: "transparent",
                fontSize: 16,
                minHeight: 44,
                color: "var(--ink)",
              }}
            />
          </span>
        </label>
      </div>

      {/* Projected balance chart — full available width, simplified */}
      <MobileSection
        title="Projected balance"
        description={forecast.firstBreachDate ? `Dips below buffer on ${shortDate(forecast.firstBreachDate)}` : `Stays above buffer — lowest ${money(forecast.lowestBalanceCents, cur)} on ${shortDate(forecast.lowestDate)}`}
      >
        <div style={{ border: "1px solid var(--line)", borderRadius: 16, background: "var(--surface)", padding: 12, overflow: "hidden" }}>
          <ProjectedBalanceChart forecast={forecast} currency={currency} />
          {forecast.firstBreachDate ? (
            <div
              style={{
                marginTop: 12,
                display: "flex",
                gap: 8,
                padding: 10,
                borderRadius: 12,
                background: "var(--surface-2)",
                border: "1px solid var(--line)",
                fontSize: 13,
                lineHeight: 1.5,
                color: "var(--ink)",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  flexShrink: 0,
                  width: 20,
                  height: 20,
                  borderRadius: "50%",
                  background: "var(--warning)",
                  color: "var(--ink)",
                  display: "grid",
                  placeItems: "center",
                  fontSize: 12,
                  fontWeight: 700,
                }}
              >
                !
              </span>
              <span>
                Your balance dips to <b className="money">{money(forecast.lowestBalanceCents, cur)}</b> on <b>{shortDate(forecast.lowestDate)}</b>
                {forecast.firstBreachDate !== forecast.lowestDate ? `, first crossing on ${shortDate(forecast.firstBreachDate)}` : ""}.
              </span>
            </div>
          ) : (
            <div
              style={{
                marginTop: 12,
                display: "flex",
                gap: 8,
                padding: 10,
                borderRadius: 12,
                background: "var(--surface-2)",
                border: "1px solid var(--line)",
                fontSize: 13,
                lineHeight: 1.5,
                color: "var(--ink-mute)",
              }}
            >
              <span
                aria-hidden="true"
                style={{
                  flexShrink: 0,
                  width: 20,
                  height: 20,
                  borderRadius: "50%",
                  background: "var(--accent)",
                  color: "var(--accent-ink)",
                  display: "grid",
                  placeItems: "center",
                  fontSize: 12,
                  fontWeight: 700,
                }}
              >
                ✓
              </span>
              <span>Stays above {bufferCents > 0 ? "your buffer" : "zero"} the whole window.</span>
            </div>
          )}
        </div>
      </MobileSection>

      {/* Events list as MobileList (date · label · amount) */}
      <MobileSection title="Upcoming in this window" description={`${forecast.upcomingEvents.length} events in the next ${horizon} days`}>
        {forecast.upcomingEvents.length === 0 ? (
          <div
            style={{
              padding: 16,
              border: "1px solid var(--line)",
              borderRadius: 16,
              background: "var(--surface)",
              color: "var(--ink-mute)",
              fontSize: 13,
              lineHeight: 1.5,
            }}
          >
            No recurring bills, income, or planned items detected in this window yet.
          </div>
        ) : (
          <MobileList ariaLabel="Upcoming cashflow events">
            {forecast.upcomingEvents.map((e, i) => {
              const inflow = e.amountCents > 0;
              return (
                <MobileListItem
                  key={`${e.date}-${e.label}-${i}`}
                  icon={
                    <span
                      aria-hidden="true"
                      style={{
                        width: 10,
                        height: 10,
                        borderRadius: "50%",
                        background: eventDotColor(e),
                        display: "inline-block",
                        flexShrink: 0,
                      }}
                    />
                  }
                  title={e.label}
                  subtitle={shortDate(e.date)}
                  value={money(e.amountCents, cur)}
                  valueTone={inflow ? "positive" : "default"}
                  chevron={false}
                />
              );
            })}
          </MobileList>
        )}
      </MobileSection>

      {/* Good to know — progressive disclosure: one insight at a time */}
      {(cautions.length > 0 || infos.length > 0) && (
        <MobileSection title="Good to know">
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {cautions.map((w, i) => (
              <div
                key={`c-${i}`}
                style={{
                  display: "flex",
                  gap: 8,
                  padding: 12,
                  borderRadius: 12,
                  background: "var(--surface)",
                  border: "1px solid var(--line)",
                  fontSize: 13,
                  lineHeight: 1.5,
                  color: "var(--ink)",
                }}
              >
                <span
                  aria-hidden="true"
                  style={{
                    flexShrink: 0,
                    width: 20,
                    height: 20,
                    borderRadius: "50%",
                    background: "var(--warning)",
                    display: "grid",
                    placeItems: "center",
                    fontSize: 12,
                    fontWeight: 700,
                  }}
                >
                  !
                </span>
                <span>{blurAmounts(w.message)}</span>
              </div>
            ))}
            {infos.map((w, i) => (
              <div
                key={`i-${i}`}
                style={{
                  display: "flex",
                  gap: 8,
                  padding: 12,
                  borderRadius: 12,
                  background: "var(--surface-2)",
                  border: "1px solid var(--line)",
                  fontSize: 13,
                  lineHeight: 1.5,
                  color: "var(--ink-mute)",
                }}
              >
                <span
                  aria-hidden="true"
                  style={{
                    flexShrink: 0,
                    width: 20,
                    height: 20,
                    borderRadius: "50%",
                    background: "var(--line)",
                    display: "grid",
                    placeItems: "center",
                    fontSize: 12,
                    fontWeight: 700,
                  }}
                >
                  i
                </span>
                <span>{blurAmounts(w.message)}</span>
              </div>
            ))}
          </div>
        </MobileSection>
      )}
    </div>
  );
}
