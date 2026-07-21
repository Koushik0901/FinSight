import { useState } from "react";
import { useCashflowForecast } from "../api/hooks/cashflow";
import { useFinancialMetrics } from "../api/hooks/metrics";
import type { CashflowForecast, CashflowEvent } from "../api/client";
import { money } from "../utils/format";

const HORIZONS = [30, 60, 90] as const;

/** Parse a "YYYY-MM-DD" into a short "Mon D" label. */
function shortDate(iso: string): string {
  const d = new Date(`${iso}T00:00:00`);
  return d.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

/** Dollars → cents for the buffer/test-spend inputs. Empty/invalid → 0. */
function toCents(dollars: string): number {
  const n = Number(dollars.replace(/[^0-9.]/g, ""));
  return Number.isFinite(n) ? Math.round(n * 100) : 0;
}

/** The currency's symbol ("$", "€", "CA$"…) for input adornments, derived from
 *  the code so it's correct for any currency rather than assuming a dollar. */
function currencySymbol(currency?: string): string {
  try {
    const parts = new Intl.NumberFormat(undefined, { style: "currency", currency: currency ?? "USD" }).formatToParts(0);
    return parts.find((p) => p.type === "currency")?.value ?? "$";
  } catch {
    return "$";
  }
}

function ProjectedBalanceChart({ forecast, currency }: { forecast: CashflowForecast; currency?: string }) {
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

  const linePath = days.map((d, i) => `${i === 0 ? "M" : "L"}${x(i).toFixed(1)},${y(d.projectedBalanceCents).toFixed(1)}`).join(" ");
  const areaPath = `${linePath} L${W},${H} L0,${H} Z`;
  const bufferY = y(buffer);
  const lowIdx = Math.max(0, days.findIndex((d) => d.date === forecast.lowestDate));

  // Map each dated event to its day index for a marker.
  const dayIndex = new Map(days.map((d, i) => [d.date, i]));
  const markers = forecast.upcomingEvents
    .map((e) => ({ e, i: dayIndex.get(e.date) }))
    .filter((m): m is { e: CashflowEvent; i: number } => m.i !== undefined);

  return (
    <div style={{ position: "relative" }}>
      <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={H} preserveAspectRatio="none" style={{ display: "block", marginTop: 8, overflow: "visible" }} role="img" aria-label={`Projected balance over ${forecast.horizonDays} days, lowest ${money(forecast.lowestBalanceCents, currency ? { currency } : undefined)} on ${shortDate(forecast.lowestDate)}`}>
        <defs>
          <linearGradient id="cf-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="rgba(201,249,80,0.20)" />
            <stop offset="1" stopColor="rgba(201,249,80,0.00)" />
          </linearGradient>
        </defs>
        {/* Zero line, if the range crosses it — a negative balance is an overdraft. */}
        {lo < 0 && hi > 0 && <line x1="0" y1={y(0)} x2={W} y2={y(0)} stroke="var(--negative)" strokeWidth="1" opacity="0.5" />}
        {/* Buffer line */}
        <line x1="0" y1={bufferY} x2={W} y2={bufferY} stroke="var(--warning)" strokeWidth="1" strokeDasharray="5 5" opacity="0.75" />
        <path d={areaPath} fill="url(#cf-fill)" />
        <path d={linePath} fill="none" stroke="var(--accent)" strokeWidth="2" />
        {/* Event markers: income above, outflow below the line. */}
        {markers.map(({ e, i }, k) => (
          <circle
            key={k}
            cx={x(i)}
            cy={y(days[i]!.projectedBalanceCents)}
            r={3.5}
            fill={e.kind === "income" ? "var(--positive)" : e.kind === "hypothetical" ? "var(--warning)" : "var(--ink-faint)"}
            stroke="var(--bg)"
            strokeWidth="1.5"
          />
        ))}
        {/* Lowest point */}
        <circle cx={x(lowIdx)} cy={y(forecast.lowestBalanceCents)} r={5} fill="var(--negative)" stroke="var(--bg)" strokeWidth="2" />
      </svg>
      <div className="chart-legend">
        <span className="lg"><span className="sw" style={{ background: "var(--accent)" }} />Projected balance</span>
        <span className="lg"><span className="dot" style={{ background: "var(--negative)" }} />Lowest point</span>
        <span className="lg"><span className="dot" style={{ background: "var(--positive)" }} />Income</span>
        <span className="lg"><span className="sw" style={{ background: "var(--warning)" }} />Buffer {money(buffer, currency ? { currency } : undefined)}</span>
      </div>
    </div>
  );
}

export default function Cashflow() {
  const [horizon, setHorizon] = useState<(typeof HORIZONS)[number]>(30);
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

  return (
    <div className="screen cashflow">
      <div>
        <div className="eyebrow"><span className="dot" />Plan · next {horizon} days</div>
        <h1 className="h3" style={{ margin: "4px 0 0" }}>Cash flow &amp; safe to spend</h1>
      </div>

      {isError ? (
        <div className="card"><p className="muted" style={{ margin: 0 }}>Couldn&rsquo;t build the forecast right now.</p></div>
      ) : isLoading && !forecast ? (
        <div className="card"><p className="muted" style={{ margin: 0 }}>Projecting your next {horizon} days…</p></div>
      ) : forecast ? (
        <>
          <section className="card cf-hero">
            <div className="cf-hero-main">
              <div className="eyebrow">Safe to spend now</div>
              <div className="cf-sts num money">{money(forecast.safeToSpendCents, cur)}</div>
              <p className="cf-sts-sub">
                What you can spend today and still keep {bufferCents > 0 ? `at least your ${money(bufferCents, cur)} buffer` : "a positive balance"} on every day through {shortDate(forecast.days[forecast.days.length - 1]!.date)} — after your bills, subscriptions, and everyday spending.
                {!forecast.reliable && " This is a rough estimate — there isn't much history yet."}
              </p>
            </div>
            <div className="cf-controls">
              <div className="cf-ctl">
                <label>Horizon</label>
                <div className="seg" role="group" aria-label="Forecast horizon">
                  {HORIZONS.map((h) => (
                    <button key={h} type="button" className={h === horizon ? "on" : ""} aria-pressed={h === horizon} onClick={() => setHorizon(h)}>{h}d</button>
                  ))}
                </div>
              </div>
              <div className="cf-ctl">
                <label htmlFor="cf-buffer">Keep a safety buffer of</label>
                <div className="money-in"><span>{currencySymbol(currency)}</span><input id="cf-buffer" inputMode="decimal" value={bufferInput} onChange={(e) => setBufferInput(e.target.value)} /></div>
              </div>
              <div className="cf-ctl">
                <label htmlFor="cf-test">Test a purchase</label>
                <div className="money-in"><span>{currencySymbol(currency)}</span><input id="cf-test" inputMode="decimal" placeholder="0" value={testInput} onChange={(e) => setTestInput(e.target.value)} /></div>
              </div>
            </div>
          </section>

          <section className="card">
            <div className="eyebrow">Projected balance</div>
            <ProjectedBalanceChart forecast={forecast} currency={currency} />
            {forecast.firstBreachDate ? (
              <div className="explain-warn caution" style={{ marginTop: 14 }}>
                <span className="explain-warn-ic" aria-hidden="true">!</span>
                <span>Your balance dips to <b className="money">{money(forecast.lowestBalanceCents, cur)}</b> on <b>{shortDate(forecast.lowestDate)}</b>, {bufferCents > 0 ? "below your buffer" : "its lowest"} — the tight point{forecast.firstBreachDate !== forecast.lowestDate ? `, first crossing the line on ${shortDate(forecast.firstBreachDate)}` : ""}.</span>
              </div>
            ) : (
              <div className="explain-warn info" style={{ marginTop: 14 }}>
                <span className="explain-warn-ic" aria-hidden="true">i</span>
                <span>Your balance stays above {bufferCents > 0 ? "your buffer" : "zero"} the whole window — lowest is <b className="money">{money(forecast.lowestBalanceCents, cur)}</b> on {shortDate(forecast.lowestDate)}.</span>
              </div>
            )}
          </section>

          <div className="cf-grid">
            <section className="card">
              <div className="eyebrow">Upcoming in this window</div>
              {forecast.upcomingEvents.length === 0 ? (
                <p className="muted" style={{ marginTop: 10 }}>No recurring bills, income, or planned items detected in this window yet.</p>
              ) : (
                <div style={{ marginTop: 10 }}>
                  {forecast.upcomingEvents.map((e, i) => {
                    const inflow = e.amountCents > 0;
                    const color = inflow ? "var(--positive)" : e.kind === "hypothetical" ? "var(--warning)" : "var(--ink-faint)";
                    return (
                      <div key={i} className="cf-evrow">
                        <div className="cf-evdate num">{shortDate(e.date)}</div>
                        <span className="cf-evdot" style={{ background: color }} />
                        <div className="cf-evlabel">{e.label}</div>
                        <div className={`cf-evamt num money${inflow ? " in" : ""}`}>{money(e.amountCents, cur)}</div>
                      </div>
                    );
                  })}
                </div>
              )}
            </section>

            <section className="card">
              <div className="eyebrow">Good to know</div>
              {cautions.length === 0 && infos.length === 0 ? (
                <p className="muted" style={{ marginTop: 10 }}>Nothing else stands out in this window.</p>
              ) : (
                <>
                  {cautions.map((w, i) => (
                    <div key={`c-${i}`} className="explain-warn caution" style={{ marginTop: 10 }}>
                      <span className="explain-warn-ic" aria-hidden="true">!</span>
                      <span>{w.message}</span>
                    </div>
                  ))}
                  {infos.map((w, i) => (
                    <div key={`i-${i}`} className="explain-warn info" style={{ marginTop: 10 }}>
                      <span className="explain-warn-ic" aria-hidden="true">i</span>
                      <span>{w.message}</span>
                    </div>
                  ))}
                </>
              )}
            </section>
          </div>
        </>
      ) : null}
    </div>
  );
}
