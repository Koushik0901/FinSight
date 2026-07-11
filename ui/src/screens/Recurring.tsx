import { useEffect, useMemo, useState } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import EmptyState from "../components/EmptyState";
import { useRecurring } from "../api/hooks/recurring";
import { usePlannedTransactions } from "../api/hooks/plannedTransactions";
import type { PlannedTransaction } from "../api/client";
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

export default function Recurring() {
  const navigate = useNavigate();
  const { data: items = [], isLoading, error } = useRecurring();
  const { data: plannedTransactions = [] } = usePlannedTransactions();
  const [searchParams, setSearchParams] = useSearchParams();
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

  const totalMonthlyCommitted = items
    .filter((item) => item.lastAmountCents < 0)
    .reduce((sum, item) => sum + monthlyEquivalentCents(item), 0);
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
        <div className="muted" style={{ marginTop: 8 }}>per month in fixed commitments</div>
      </div>

      <div className="stat-row">
        <div className="stat"><div className="label">Bills</div><div className="value">{billsCount}</div><div className="sub">Regular essentials</div></div>
        <div className="stat"><div className="label">Subscriptions</div><div className="value">{subscriptionsCount}</div><div className="sub">Agent-detected subscriptions</div></div>
        <div className="stat"><div className="label">Income</div><div className="value">{incomeCount}</div><div className="sub">Recurring inflows</div></div>
        <div className="stat accent"><div className="label">Next 7 days</div><div className="value">{nextSevenDays}</div><div className="sub">Upcoming expected hits</div></div>
      </div>

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
                  ...group.items.map((item) => (
                    <tr key={`${group.label}-${item.merchantRaw}-${item.nextExpected}`} title={(item.reasons ?? []).join(" · ")}>
                      <td><div className="row row-sm"><span className="cswatch" style={{ background: item.categoryColor || (item.lastAmountCents > 0 ? "var(--accent)" : "var(--ink-faint)") }} /><div><div>{prettyMerchant(item.merchantRaw)}</div><div className="muted" style={{ fontSize: 12 }}>{item.categoryLabel || group.label} · {item.occurrences}× · {Math.round((item.confidence ?? 0) * 100)}% confidence</div></div></div></td>
                      <td><span className="chip">{recurringFrequency(item)}</span></td>
                      <td><span className="mono muted">{new Date(item.nextExpected).toLocaleDateString("en-US", { month: "short", day: "numeric" })}</span></td>
                      <td className="right"><span className={`money ${item.lastAmountCents > 0 ? "pos" : ""}`}>{money(item.lastAmountCents, { decimals: 2 })}</span></td>
                    </tr>
                  )),
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
