import { useEffect, useMemo, useState } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import EmptyState from "../components/EmptyState";
import {
  useRecurring,
  useSetSubscriptionVerdict,
  useSetSubscriptionTrial,
  useMarkSubscriptionCancelled,
} from "../api/hooks/recurring";
import { usePlannedTransactions } from "../api/hooks/plannedTransactions";
import type { PlannedTransaction, RecurringItem } from "../api/client";
import { money } from "../utils/format";
import { prettyMerchant } from "../utils/merchant";
import { recurringFrequency, monthlyEquivalentCents } from "../utils/recurring";
import PlannedTransactionDrawer from "../components/PlannedTransactionDrawer";

function recurringGroup(item: { kind: string; lastAmountCents: number }) {
  // Group by the deterministically-classified kind (Phase 6). Falls back to
  // amount sign for older/edge cases.
  if (item.kind === "income" || item.lastAmountCents > 0) return "Income";
  if (item.kind === "subscription") return "Subscriptions";
  return "Bills";
}

/** Colored ±% pill for a detected price change (#58). Up = negative tone. */
function PriceChangePill({ pc, compact = false }: { pc: NonNullable<RecurringItem["priceChange"]>; compact?: boolean }) {
  const up = pc.toCents >= pc.fromCents;
  const tone = up ? "var(--negative)" : "var(--positive)";
  return (
    <span
      className="chip"
      title={`Was ${money(pc.fromCents, { decimals: 2 })}, now ${money(pc.toCents, { decimals: 2 })} since ${new Date(pc.effectiveDate).toLocaleDateString("en-US", { month: "short", day: "numeric" })}`}
      style={{ color: tone, borderColor: tone, background: "transparent", fontVariantNumeric: "tabular-nums", ...(compact ? { fontSize: 11, padding: "1px 7px", marginLeft: 6 } : {}) }}
    >
      {up ? "↑" : "↓"} {pc.pct >= 0 ? "+" : ""}{Math.round(pc.pct)}%
    </span>
  );
}

function fmtShortDate(d: string) {
  return new Date(d).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" });
}

/**
 * Subscription lifecycle (#75): trial / cancelled badges plus the affordances to
 * record them. Marking a trial schedules a heads-up before it converts; marking
 * cancelled stops ongoing alerts and surfaces any charge dated after the cancel
 * date. Only shown for genuine subscriptions.
 */
function SubscriptionLifecycle({ item }: { item: RecurringItem }) {
  const setTrial = useSetSubscriptionTrial();
  const markCancelled = useMarkSubscriptionCancelled();
  const [form, setForm] = useState<null | "trial" | "cancelled">(null);
  const [date, setDate] = useState("");
  if (item.kind !== "subscription") return null;
  const label = prettyMerchant(item.merchantRaw);
  const busy = setTrial.isPending || markCancelled.isPending;
  const cancelled = item.verdict === "cancelled";

  const submit = () => {
    if (!date) return;
    if (form === "trial") setTrial.mutate({ merchantKey: item.merchantKey, label, trialEndsAt: date });
    else markCancelled.mutate({ merchantKey: item.merchantKey, label, cancelledAt: date });
    setForm(null);
    setDate("");
  };

  const linkBtn = { background: "none", border: "none", padding: 0, font: "inherit", color: "var(--ink-mute)", textDecoration: "underline", cursor: "pointer" } as const;

  return (
    <div className="row row-sm" style={{ marginTop: 5, fontSize: 12, gap: 8, flexWrap: "wrap", alignItems: "center" }}>
      {item.trialEndsAt && <span className="chip" style={{ fontSize: 11, padding: "1px 8px", color: "var(--accent)", borderColor: "var(--accent)" }}>Trial ends {fmtShortDate(item.trialEndsAt)}</span>}
      {cancelled && <span className="chip warning" style={{ fontSize: 11, padding: "1px 8px" }}>Cancelled{item.cancelledAt ? ` ${fmtShortDate(item.cancelledAt)}` : ""}</span>}
      {form === null ? (
        <>
          <button type="button" style={linkBtn} disabled={busy} onClick={() => { setForm("trial"); setDate(item.trialEndsAt ?? ""); }}>{item.trialEndsAt ? "Edit trial" : "Mark as trial"}</button>
          {item.trialEndsAt && <button type="button" style={linkBtn} disabled={busy} onClick={() => setTrial.mutate({ merchantKey: item.merchantKey, label, trialEndsAt: null })}>Clear trial</button>}
          {!cancelled && <button type="button" style={linkBtn} disabled={busy} onClick={() => { setForm("cancelled"); setDate(new Date().toISOString().slice(0, 10)); }}>I cancelled this</button>}
        </>
      ) : (
        <span className="row row-sm" style={{ gap: 6, alignItems: "center" }}>
          <span className="muted">{form === "trial" ? "Trial ends" : "Cancelled on"}</span>
          <input type="date" value={date} onChange={(e) => setDate(e.target.value)} aria-label={form === "trial" ? "Trial end date" : "Cancellation date"} style={{ fontSize: 12 }} />
          <button type="button" className="btn primary sm" disabled={!date || busy} onClick={submit}>Save</button>
          <button type="button" style={linkBtn} onClick={() => { setForm(null); setDate(""); }}>Cancel</button>
        </span>
      )}
    </div>
  );
}

export default function Recurring() {
  const navigate = useNavigate();
  const { data: items = [], isLoading, error } = useRecurring();
  const { data: plannedTransactions = [] } = usePlannedTransactions();
  const setVerdict = useSetSubscriptionVerdict();
  const [searchParams, setSearchParams] = useSearchParams();

  // Detected price changes the user hasn't yet confirmed or dismissed (#58).
  const pendingChanges = items.filter((item) => item.priceChange && !item.verdict);
  const [view, setView] = useState<"monthly" | "upcoming" | "all">("monthly");
  const [editingPlanned, setEditingPlanned] = useState<PlannedTransaction | null>(null);

  const groups = useMemo(() => {
    const filtered = items.filter((item) => {
      if (view === "monthly") return recurringFrequency(item) === "monthly" || recurringFrequency(item) === "annual" || recurringFrequency(item) === "biweekly";
      if (view === "upcoming") return new Date(item.nextExpected).getTime() <= Date.now() + 7 * 86400000;
      return true;
    });
    return ["Bills", "Subscriptions", "Income"].map((label) => ({ label, items: filtered.filter((item) => recurringGroup(item) === label) }));
  }, [items, view]);

  // Only entries confident enough to be treated as real obligations, matching
  // exactly what the budget planner and Copilot count. A headline built from
  // every guess on the list would disagree with the plan built from the same
  // data, and the user would have no way to tell which was right.
  const committedItems = items.filter((item) => item.feedsProjections);
  const totalMonthlyCommitted = committedItems.reduce(
    (sum, item) => sum + monthlyEquivalentCents(item),
    0,
  );
  const uncountedCount = items.filter(
    (item) => item.lastAmountCents < 0 && !item.feedsProjections,
  ).length;
  const billsCount = items.filter((item) => recurringGroup(item) === "Bills").length;
  const subscriptionsCount = items.filter((item) => recurringGroup(item) === "Subscriptions").length;
  const incomeCount = items.filter((item) => recurringGroup(item) === "Income").length;
  const nextSevenDays = items.filter((item) => new Date(item.nextExpected).getTime() <= Date.now() + 7 * 86400000).length;
  const activePlanned = plannedTransactions.filter((item) => item.status === "planned");

  useEffect(() => {
    const focus = searchParams.get("focusPlanned");
    if (!focus || editingPlanned) return;
    const target = plannedTransactions.find((item) => item.id === focus || item.description.toLowerCase() === focus.toLowerCase());
    if (!target) return;
    setEditingPlanned(target);
    const next = new URLSearchParams(searchParams);
    next.delete("focusPlanned");
    setSearchParams(next, { replace: true });
  }, [editingPlanned, plannedTransactions, searchParams, setSearchParams]);

  if (isLoading) return <div className="stub">Loading recurring items…</div>;
  if (error) return <div className="stub" role="alert">Error loading recurring items.</div>;

  if (items.length === 0 && activePlanned.length === 0) {
    return (
      <div className="screen screen-recurring">
        <EmptyState
          title="No recurring items yet"
          description="Import a few months of statements and FinSight detects your subscriptions, bills, and recurring income automatically."
          actions={<button className="btn primary" type="button" onClick={() => navigate("/onboarding")}>Import transactions</button>}
        />
      </div>
    );
  }

  return (
    <div className="screen screen-recurring">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot" />Recurring · {items.length} items · {subscriptionsCount} subscriptions</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>What happens every month.</h1>
        </div>
        <div className="toolbar">
          <button className={view === "monthly" ? "on" : ""} type="button" onClick={() => setView("monthly")}>Monthly</button>
          <button className={view === "upcoming" ? "on" : ""} type="button" onClick={() => setView("upcoming")}>Upcoming</button>
          <button className={view === "all" ? "on" : ""} type="button" onClick={() => setView("all")}>All</button>
        </div>
      </div>

      <div className="card accent" style={{ padding: 28 }}>
        <div className="eyebrow"><span className="dot" />Monthly committed</div>
        <div className="figure money" style={{ fontSize: 52, lineHeight: 1, marginTop: 10 }}>{money(totalMonthlyCommitted)}</div>
        <div className="muted" style={{ marginTop: 8 }}>
          per month in fixed commitments
          {uncountedCount > 0
            ? ` · ${uncountedCount} less certain ${uncountedCount === 1 ? "entry is" : "entries are"} listed below but not counted`
            : ""}
        </div>
      </div>

      <div className="stat-row">
        <div className="stat"><div className="label">Bills</div><div className="value">{billsCount}</div><div className="sub">Regular essentials</div></div>
        <div className="stat"><div className="label">Subscriptions</div><div className="value">{subscriptionsCount}</div><div className="sub">Agent-detected subscriptions</div></div>
        <div className="stat"><div className="label">Income</div><div className="value">{incomeCount}</div><div className="sub">Recurring inflows</div></div>
        <div className="stat accent"><div className="label">Next 7 days</div><div className="value">{nextSevenDays}</div><div className="sub">Upcoming expected hits</div></div>
      </div>

      {pendingChanges.length > 0 && (
        <section className="section">
          <div className="card" style={{ padding: 20 }}>
            <div className="eyebrow"><span className="dot" style={{ background: "var(--warning)" }} />Changes to review · {pendingChanges.length}</div>
            <div className="muted" style={{ fontSize: 13, margin: "6px 0 2px" }}>
              Price moves we spotted in your recurring charges. Confirm what&apos;s real, dismiss what isn&apos;t.
            </div>
            {pendingChanges.map((item) => {
              const pc = item.priceChange!;
              return (
                <div key={item.merchantKey} className="row" style={{ justifyContent: "space-between", gap: 14, padding: "14px 0", borderTop: "1px solid var(--hairline)", flexWrap: "wrap" }}>
                  <div style={{ minWidth: 0 }}>
                    <div style={{ fontWeight: 600 }}>{prettyMerchant(item.merchantRaw)} <PriceChangePill pc={pc} /></div>
                    <div className="muted" style={{ fontSize: 12, marginTop: 3 }}>
                      <span className="money">{money(pc.fromCents, { decimals: 2 })} → {money(pc.toCents, { decimals: 2 })}</span>
                      {" · since "}{new Date(pc.effectiveDate).toLocaleDateString("en-US", { month: "short", day: "numeric" })}
                      {" · "}{Math.round((item.confidence ?? 0) * 100)}% confidence
                    </div>
                  </div>
                  <div className="row row-sm">
                    <button className="btn primary sm" type="button" disabled={setVerdict.isPending} onClick={() => setVerdict.mutate({ merchantKey: item.merchantKey, verdict: "confirmed" })}>Confirm</button>
                    <button className="btn outline sm" type="button" disabled={setVerdict.isPending} onClick={() => setVerdict.mutate({ merchantKey: item.merchantKey, verdict: "dismissed" })}>Dismiss</button>
                  </div>
                </div>
              );
            })}
          </div>
        </section>
      )}

      <section className="section">
        <div className="card flush">
          <table className="tbl">
            <thead>
              <tr>
                <th>Name</th>
                <th>Frequency</th>
                <th>Next date</th>
                <th className="right">Amount</th>
              </tr>
            </thead>
            <tbody>
              {groups.map((group) => (
                group.items.length > 0 ? [
                  <tr key={`${group.label}-hdr`}>
                    <td colSpan={4} style={{ paddingTop: 18, paddingBottom: 10, fontSize: 12, color: "var(--ink-faint)", fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.08em" }}>{group.label}</td>
                  </tr>,
                  ...group.items.map((item) => {
                    const dismissed = item.verdict === "dismissed";
                    const canDismiss = item.kind !== "income";
                    return (
                    <tr key={`${group.label}-${item.merchantKey}`} title={(item.reasons ?? []).join(" · ")} style={dismissed ? { opacity: 0.5 } : undefined}>
                      <td><div className="row row-sm"><span className="cswatch" style={{ background: item.categoryColor || (item.lastAmountCents > 0 ? "var(--accent)" : "var(--ink-faint)") }} /><div><div>{prettyMerchant(item.merchantRaw)}{item.priceChange && <PriceChangePill pc={item.priceChange} compact />}{dismissed && <span className="chip" style={{ marginLeft: 6, fontSize: 11, padding: "1px 8px" }}>dismissed</span>}</div><div className="muted" style={{ fontSize: 12 }}>{item.categoryLabel || group.label} · {item.occurrences}× · {Math.round((item.confidence ?? 0) * 100)}% confidence{item.kind !== "income" && !item.feedsProjections ? " · not used in forecasts" : ""}{canDismiss && <>{" · "}<button type="button" aria-label={`${dismissed ? "Restore" : "Dismiss"} ${prettyMerchant(item.merchantRaw)}`} onClick={() => setVerdict.mutate({ merchantKey: item.merchantKey, verdict: dismissed ? null : "dismissed" })} disabled={setVerdict.isPending} style={{ background: "none", border: "none", padding: 0, font: "inherit", color: "var(--ink-mute)", textDecoration: "underline", cursor: "pointer" }}>{dismissed ? "Restore" : "Dismiss"}</button></>}</div><SubscriptionLifecycle item={item} /></div></div></td>
                      <td><span className="chip">{recurringFrequency(item)}</span></td>
                      <td><span className="mono muted">{new Date(item.nextExpected).toLocaleDateString("en-US", { month: "short", day: "numeric" })}</span></td>
                      <td className="right"><span className={`money ${item.lastAmountCents > 0 ? "pos" : ""}`}>{money(item.lastAmountCents, { decimals: 2 })}</span></td>
                    </tr>
                    );
                  }),
                ] : null
              ))}
            </tbody>
          </table>
        </div>
      </section>

      <section className="section">
        <div className="day-hdr" style={{ marginBottom: 14 }}>
          <div>
            <div className="eyebrow"><span className="dot" />Planned transactions · {activePlanned.length}</div>
            <h2 className="h1" style={{ fontSize: 22, marginTop: 4 }}>What needs a date and a decision.</h2>
          </div>
        </div>
        <div className="card flush">
          {activePlanned.length === 0 ? (
            <div className="muted" style={{ padding: 18 }}>No planned transactions yet.</div>
          ) : (
            activePlanned.map((item, index) => (
              <button
                key={item.id}
                type="button"
                onClick={() => setEditingPlanned(item)}
                style={{
                  width: "100%",
                  textAlign: "left",
                  display: "grid",
                  gridTemplateColumns: "1fr auto auto",
                  gap: 14,
                  alignItems: "center",
                  padding: "14px 16px",
                  borderBottom: index === activePlanned.length - 1 ? "none" : "1px solid var(--hairline)",
                  background: "transparent",
                }}
              >
                <div>
                  <div>{item.description}</div>
                  <div className="muted" style={{ fontSize: 12 }}>
                    {item.accountId ? "Linked account" : "No linked account"} · {item.categoryId ? "linked category" : "uncategorized"}
                  </div>
                </div>
                <span className="chip">{new Date(item.dueDate).toLocaleDateString("en-US", { month: "short", day: "numeric" })}</span>
                <span className={`money ${item.amountCents > 0 ? "pos" : ""}`}>{money(item.amountCents, { decimals: 2 })}</span>
              </button>
            ))
          )}
        </div>
      </section>

      <PlannedTransactionDrawer open={editingPlanned !== null} onClose={() => setEditingPlanned(null)} planned={editingPlanned} />
    </div>
  );
}
