import { useState, useMemo, useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { commands, type RecurringItem } from "../api/client";
import * as I from "../components/Icons";

function useRecurring() {
  return useQuery<RecurringItem[]>({
    queryKey: ["recurring"],
    queryFn: async () => {
      const result = await commands.listRecurring();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
  });
}

function fmt(cents: number) {
  const abs = Math.abs(cents);
  const s = new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: abs % 100 === 0 ? 0 : 2 }).format(abs / 100);
  return cents < 0 ? s : `+${s}`;
}

function initials(name: string) {
  return name.split(/\s+/).map((w) => w[0]).join("").toUpperCase().slice(0, 2);
}

function colorFromStr(s: string) {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = ((h << 5) - h + s.charCodeAt(i)) | 0;
  const hue = Math.abs(h) % 360;
  return `hsl(${hue},60%,42%)`;
}

type View = "calendar" | "list" | "subs";

// ── Calendar ─────────────────────────────────────────────────────────────

function CalendarView({ items }: { items: RecurringItem[] }) {
  const now = new Date();
  const [offset, setOffset] = useState(0); // months offset from current
  const [selectedDay, setSelectedDay] = useState<number | null>(null);
  useEffect(() => { setSelectedDay(null); }, [offset]);

  const year = now.getFullYear() + Math.floor((now.getMonth() + offset) / 12);
  const month = ((now.getMonth() + offset) % 12 + 12) % 12;
  const firstDay = new Date(year, month, 1).getDay(); // 0=Sun
  const daysInMonth = new Date(year, month + 1, 0).getDate();
  const monthLabel = new Date(year, month, 1).toLocaleString("default", { month: "long", year: "numeric" });

  // Build expected-day → items map
  const dayMap = useMemo(() => {
    const m: Record<number, RecurringItem[]> = {};
    items.forEach((item) => {
      const nextDate = new Date(item.nextExpected + "T00:00:00");
      if (nextDate.getFullYear() === year && nextDate.getMonth() === month) {
        const d = nextDate.getDate();
        (m[d] ??= []).push(item);
      }
    });
    return m;
  }, [items, year, month]);

  const totalOut = Object.values(dayMap).flat().filter((i) => i.lastAmountCents < 0).reduce((s, i) => s + i.lastAmountCents, 0);
  const totalIn  = Object.values(dayMap).flat().filter((i) => i.lastAmountCents > 0).reduce((s, i) => s + i.lastAmountCents, 0);

  const today = now.getDate();
  const isCurrentMonth = year === now.getFullYear() && month === now.getMonth();

  return (
    <div className="rcal">
      <div className="rcal-head">
        <div>
          <div className="rcal-summary">
            <span>{monthLabel}</span>
            {totalIn > 0 && <span className="rcal-in"> · <b>{fmt(totalIn)}</b> in</span>}
            {totalOut < 0 && <span className="rcal-out"> · <b>{fmt(Math.abs(totalOut))}</b> out</span>}
          </div>
        </div>
        <div className="rcal-nav">
          <button className="rcal-arrow" onClick={() => setOffset((o) => o - 1)} aria-label="Previous month"><I.ArrowLeft /></button>
          {offset !== 0 && (
            <button className="rcal-today" onClick={() => setOffset(0)}>Today</button>
          )}
          <button className="rcal-arrow" onClick={() => setOffset((o) => o + 1)} aria-label="Next month"><I.ArrowRight /></button>
        </div>
      </div>

      {/* Day-of-week headers */}
      <div className="rcal-weekdays">
        {["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"].map((d, i) => (
          <div key={d} className={`rcal-dow${i === 0 || i === 6 ? " weekend" : ""}`}>{d}</div>
        ))}
      </div>

      {/* Calendar grid */}
      <div className="rcal-grid">
        {/* Leading empty cells */}
        {Array.from({ length: firstDay }, (_, i) => (
          <div key={`empty-${i}`} className="rcal-cell empty" />
        ))}
        {Array.from({ length: daysInMonth }, (_, i) => {
          const day = i + 1;
          const dayItems = dayMap[day] ?? [];
          const isToday = isCurrentMonth && day === today;
          const isPast  = isCurrentMonth && day < today;
          const isWeekend = [0, 6].includes(new Date(year, month, day).getDay());
          const netCents = dayItems.reduce((s, r) => s + r.lastAmountCents, 0);
          const loadPct = dayItems.length > 0 ? Math.min(100, Math.abs(netCents) / 200) : 0;

          return (
            <div
              key={day}
              className={[
                "rcal-cell",
                isToday ? "today" : "",
                isPast  ? "past"  : "",
                isWeekend && !isToday ? "weekend" : "",
                netCents > 0 ? "pos" : "",
                selectedDay === day ? "selected" : "",
              ].filter(Boolean).join(" ")}
              style={{ "--load": `${loadPct}%`, cursor: dayItems.length > 0 ? "pointer" : undefined } as React.CSSProperties}
              onClick={() => setSelectedDay(selectedDay === day ? null : day)}
            >
              {isToday && <div className="rcal-today-glow" />}
              <div className="rcal-cell-head">
                <span className="rcal-day">{day}</span>
                {netCents !== 0 && (
                  <span className={`rcal-net${netCents > 0 ? " pos" : " neg"}`}>
                    {fmt(netCents)}
                  </span>
                )}
              </div>

              {dayItems.length > 0 && (
                <div className="rcal-dots">
                  {dayItems.slice(0, 3).map((item) => (
                    <div
                      key={item.merchantRaw}
                      className={`rcal-dot${item.lastAmountCents > 0 ? " income" : ""}`}
                      style={{
                        background: item.lastAmountCents > 0 ? "var(--accent)" : (item.categoryColor || colorFromStr(item.merchantRaw)),
                      }}
                      title={`${item.merchantRaw}: ${fmt(item.lastAmountCents)}`}
                    >
                      {initials(item.merchantRaw)}
                    </div>
                  ))}
                  {dayItems.length > 3 && (
                    <div className="rcal-dot rcal-more">+{dayItems.length - 3}</div>
                  )}
                </div>
              )}

              {isToday && <div className="rcal-today-pip">TODAY</div>}
              {dayItems.length > 0 && <div className="rcal-load" />}
            </div>
          );
        })}
      </div>
      {selectedDay !== null && (dayMap[selectedDay] ?? []).length > 0 && (() => {
        const detailItems = dayMap[selectedDay] ?? [];
        const netCents = detailItems.reduce((s, r) => s + r.lastAmountCents, 0);
        const dayDate = new Date(year, month, selectedDay);
        const weekday = dayDate.toLocaleString("default", { weekday: "long" });
        return (
          <div className="rcal-detail">
            <div>
              <div className="rcal-detail-day">{selectedDay}</div>
              <div className="rcal-detail-weekday">{weekday}</div>
              {isCurrentMonth && selectedDay === today && (
                <div className="rcal-detail-today-badge">TODAY</div>
              )}
            </div>
            <div>
              <div className={`rcal-detail-net ${netCents > 0 ? "pos" : "neg"}`}>
                {fmt(netCents)} net
              </div>
              <div className="rcal-detail-items">
                {detailItems.map((item) => {
                  const color = item.lastAmountCents > 0 ? "var(--accent)" : (item.categoryColor || colorFromStr(item.merchantRaw));
                  return (
                    <div key={item.merchantRaw} className="rcal-detail-item">
                      <div style={{ width: 30, height: 30, borderRadius: 7, background: color, color: "#fff", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 700, flexShrink: 0 }}>
                        {initials(item.merchantRaw)}
                      </div>
                      <div style={{ flex: 1 }}>
                        <div style={{ fontSize: 13.5, fontWeight: 500 }}>{item.merchantRaw}</div>
                        <div className="muted" style={{ fontSize: 12 }}>{item.categoryLabel || "Uncategorized"}</div>
                      </div>
                      <span className={`chip ${item.isSubscription ? "positive" : ""}`} style={{ fontSize: 11 }}>
                        {item.cadence}
                      </span>
                      <span className={`num tabular money ${item.lastAmountCents > 0 ? "pos" : ""}`} style={{ fontSize: 14, fontFamily: "var(--mono)" }}>
                        {fmt(item.lastAmountCents)}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        );
      })()}
    </div>
  );
}

// ── List view ─────────────────────────────────────────────────────────────

function ListView({ items }: { items: RecurringItem[] }) {
  return (
    <div className="card flush">
      <div className="card-head">
        <div className="h3">All recurring · {items.length}</div>
      </div>
      <table className="tbl">
        <thead>
          <tr>
            <th>Merchant</th>
            <th>Cadence</th>
            <th>Next expected</th>
            <th>Occurrences</th>
            <th className="right">Amount</th>
          </tr>
        </thead>
        <tbody>
          {items.map((r) => {
            const color = r.categoryColor || colorFromStr(r.merchantRaw);
            return (
              <tr key={r.merchantRaw}>
                <td>
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <div style={{ width: 28, height: 28, borderRadius: 7, background: color, color: "#fff", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 700, flexShrink: 0 }}>
                      {initials(r.merchantRaw)}
                    </div>
                    <div>
                      <div style={{ fontSize: 14 }}>{r.merchantRaw}</div>
                      <div className="muted" style={{ fontSize: 12 }}>{r.categoryLabel || "Uncategorized"}</div>
                    </div>
                  </div>
                </td>
                <td>
                  <span className="chip" style={{ fontSize: 11 }}>{r.cadence}</span>
                </td>
                <td>
                  <span style={{ fontSize: 13.5, fontFamily: "var(--mono)" }}>
                    {new Date(r.nextExpected + "T00:00:00").toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                  </span>
                </td>
                <td>
                  <span className="muted" style={{ fontSize: 13 }}>{r.occurrences}×</span>
                </td>
                <td className="right">
                  <span className={`num tabular money ${r.lastAmountCents > 0 ? "pos" : ""}`} style={{ fontSize: 14 }}>
                    {fmt(r.lastAmountCents)}
                  </span>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

// ── Subscriptions view ────────────────────────────────────────────────────

function SubsView({ subs }: { subs: RecurringItem[] }) {
  const monthlyTotal = subs.filter((s) => s.lastAmountCents < 0).reduce((t, s) => t + Math.abs(s.lastAmountCents), 0);
  const annualTotal  = monthlyTotal * 12;

  if (subs.length === 0) {
    return (
      <div className="card" style={{ textAlign: "center", padding: "48px 32px" }}>
        <div className="muted" style={{ fontSize: 14 }}>No subscriptions detected yet. Import a few months of transactions to see patterns here.</div>
      </div>
    );
  }

  return (
    <div>
      {/* Summary */}
      <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 14, marginBottom: 24 }}>
        <div className="stat">
          <div className="label">Subscriptions</div>
          <div className="value">{subs.length}</div>
          <div className="sub muted">detected patterns</div>
        </div>
        <div className="stat">
          <div className="label">Monthly total</div>
          <div className="value money">{new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(monthlyTotal / 100)}</div>
          <div className="sub muted">approximate</div>
        </div>
        <div className="stat accent">
          <div className="label">Annual cost</div>
          <div className="value money">{new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(annualTotal / 100)}</div>
          <div className="sub muted">if all renewed</div>
        </div>
      </div>

      {/* Subscription cards */}
      <div style={{ display: "grid", gridTemplateColumns: "repeat(auto-fill, minmax(240px, 1fr))", gap: 12 }}>
        {subs.map((s) => {
          const color = s.categoryColor || colorFromStr(s.merchantRaw);
          const nextDate = new Date(s.nextExpected + "T00:00:00");
          const daysUntil = Math.round((nextDate.getTime() - Date.now()) / 86400000);
          return (
            <div key={s.merchantRaw} className="card tight" style={{ padding: 16, borderLeft: `3px solid ${color}` }}>
              <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 10 }}>
                <div style={{ width: 32, height: 32, borderRadius: 8, background: color, color: "#fff", display: "grid", placeItems: "center", fontSize: 12, fontWeight: 700, flexShrink: 0 }}>
                  {initials(s.merchantRaw)}
                </div>
                <div>
                  <div style={{ fontSize: 14, fontWeight: 500 }}>{s.merchantRaw}</div>
                  <div className="muted" style={{ fontSize: 12 }}>{s.categoryLabel || "Uncategorized"}</div>
                </div>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                <div className="num tabular money" style={{ fontSize: 18, fontWeight: 600 }}>
                  {new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(Math.abs(s.lastAmountCents) / 100)}
                </div>
                <span className="chip" style={{ fontSize: 11 }}>{s.cadence}</span>
              </div>
              {(() => {
                const minAbs = Math.abs(s.minAmountCents);
                const maxAbs = Math.abs(s.maxAmountCents);
                const curAbs = Math.abs(s.lastAmountCents);
                const priceChanged = minAbs !== maxAbs;
                if (!priceChanged) return null;
                const priceUp = curAbs >= minAbs;
                const fmtAmt = (cents: number) => new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 2 }).format(cents / 100);
                return priceUp
                  ? <span className="chip warning" style={{ marginTop: 6, display: "inline-block" }}>↑ {fmtAmt(maxAbs)} → {fmtAmt(curAbs)}</span>
                  : <span className="chip positive" style={{ marginTop: 6, display: "inline-block" }}>↓ {fmtAmt(minAbs)} → {fmtAmt(curAbs)}</span>;
              })()}
              <div className="muted" style={{ fontSize: 12, marginTop: 8, fontFamily: "var(--mono)" }}>
                {daysUntil >= 0 ? `Next in ${daysUntil}d` : `${Math.abs(daysUntil)}d ago`} · {s.occurrences}× detected
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ── Main screen ───────────────────────────────────────────────────────────

export default function Recurring() {
  const { data: items = [], isLoading, error } = useRecurring();
  const [view, setView] = useState<View>("calendar");

  const subs = items.filter((i) => i.isSubscription);
  const monthlyOut = items.filter((i) => i.lastAmountCents < 0).reduce((s, i) => s + i.lastAmountCents, 0);
  const monthlyIn  = items.filter((i) => i.lastAmountCents > 0).reduce((s, i) => s + i.lastAmountCents, 0);

  if (isLoading) return <div className="stub">Detecting recurring patterns…</div>;
  if (error)     return <div className="stub">Error detecting recurring.</div>;

  if (items.length === 0) {
    return (
      <div className="screen">
        <div className="screen-header">
          <div className="screen-header-text">
            <div className="screen-eyebrow">Recurring</div>
            <h1>Predictable money, predictable peace of mind.</h1>
          </div>
        </div>
        <div className="card" style={{ textAlign: "center", padding: "64px 32px" }}>
          <I.Repeat style={{ color: "var(--ink-faint)", width: 32, height: 32, margin: "0 auto 16px" }} />
          <div style={{ fontSize: 18, fontWeight: 600, marginBottom: 8 }}>No recurring patterns yet</div>
          <div className="muted" style={{ fontSize: 14 }}>
            Import a few months of transactions — FinSight automatically detects recurring charges from your history.
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="screen">
      {/* Header */}
      <div className="screen-header">
        <div className="screen-header-text">
          <div className="screen-eyebrow">
            <span className="dot" />
            Recurring · {items.length} detected · {subs.length} subscriptions
          </div>
          <h1>Predictable money, predictable peace of mind.</h1>
        </div>
        <div className="toolbar">
          <button className={view === "calendar" ? "on" : ""} onClick={() => setView("calendar")}>
            Calendar
          </button>
          <button className={view === "list" ? "on" : ""} onClick={() => setView("list")}>
            List
          </button>
          <button className={view === "subs" ? "on" : ""} onClick={() => setView("subs")}>
            Subscriptions
          </button>
        </div>
      </div>

      {/* Stats */}
      <div className="stat-row" style={{ marginTop: 0 }}>
        <div className="stat">
          <div className="label">Monthly out</div>
          <div className="value money">
            {new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(Math.abs(monthlyOut) / 100)}
          </div>
          <div className="sub muted">{items.filter((i) => i.lastAmountCents < 0).length} items</div>
        </div>
        <div className="stat">
          <div className="label">Monthly in</div>
          <div className="value money">
            {new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(monthlyIn / 100)}
          </div>
          <div className="sub muted">{items.filter((i) => i.lastAmountCents > 0).length} items</div>
        </div>
        <div className="stat">
          <div className="label">Subscriptions</div>
          <div className="value">{subs.length}</div>
          <div className="sub muted">
            {new Intl.NumberFormat("en-US", { style: "currency", currency: "USD", maximumFractionDigits: 0 }).format(Math.abs(subs.reduce((s, i) => s + i.lastAmountCents, 0)) * 12 / 100)}/yr
          </div>
        </div>
        <div className="stat">
          <div className="label">Distinct patterns</div>
          <div className="value">{items.length}</div>
          <div className="sub muted">auto-detected</div>
        </div>
      </div>

      {/* View content */}
      <div className="section">
        {view === "calendar" && <CalendarView items={items} />}
        {view === "list"     && <ListView items={items} />}
        {view === "subs"     && <SubsView subs={subs} />}
      </div>
    </div>
  );
}
