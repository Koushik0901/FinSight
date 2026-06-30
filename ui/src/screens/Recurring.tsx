import { useMemo, useState } from "react";
import { useRecurring } from "../api/hooks/recurring";
import { money } from "../utils/format";

function recurringGroup(item: { isSubscription: boolean; lastAmountCents: number; categoryLabel: string }) {
  if (item.lastAmountCents > 0) return "Income";
  if (item.isSubscription) return "Subscriptions";
  return "Bills";
}

function recurringFrequency(item: { cadence: string; avgGapDays: number }) {
  if (item.cadence) return item.cadence;
  if (item.avgGapDays <= 8) return "weekly";
  if (item.avgGapDays <= 16) return "biweekly";
  if (item.avgGapDays <= 40) return "monthly";
  return "irregular";
}

export default function Recurring() {
  const { data: items = [], isLoading, error } = useRecurring();
  const [view, setView] = useState<"monthly" | "upcoming" | "all">("monthly");

  const groups = useMemo(() => {
    const filtered = items.filter((item) => {
      if (view === "monthly") return recurringFrequency(item) === "monthly" || recurringFrequency(item) === "annual" || recurringFrequency(item) === "biweekly";
      if (view === "upcoming") return new Date(item.nextExpected).getTime() <= Date.now() + 7 * 86400000;
      return true;
    });
    return ["Bills", "Subscriptions", "Income"].map((label) => ({ label, items: filtered.filter((item) => recurringGroup(item) === label) }));
  }, [items, view]);

  const totalMonthlyCommitted = items.filter((item) => item.lastAmountCents < 0).reduce((sum, item) => {
    const cadence = recurringFrequency(item);
    if (cadence === "weekly") return sum + Math.round(Math.abs(item.lastAmountCents) * 4.33);
    if (cadence === "biweekly") return sum + Math.round(Math.abs(item.lastAmountCents) * 2.16);
    if (cadence === "annual") return sum + Math.round(Math.abs(item.lastAmountCents) / 12);
    return sum + Math.abs(item.lastAmountCents);
  }, 0);
  const billsCount = items.filter((item) => recurringGroup(item) === "Bills").length;
  const subscriptionsCount = items.filter((item) => recurringGroup(item) === "Subscriptions").length;
  const incomeCount = items.filter((item) => recurringGroup(item) === "Income").length;
  const nextSevenDays = items.filter((item) => new Date(item.nextExpected).getTime() <= Date.now() + 7 * 86400000).length;

  if (isLoading) return <div className="stub">Loading recurring items…</div>;
  if (error) return <div className="stub" role="alert">Error loading recurring items.</div>;

  return (
    <div className="screen screen-recurring">
      <div className="day-hdr">
        <div>
          <div className="eyebrow">RECURRING · {items.length} ITEMS</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>What happens every month.</h1>
        </div>
        <div className="toolbar">
          <button className={view === "monthly" ? "on" : ""} type="button" onClick={() => setView("monthly")}>Monthly</button>
          <button className={view === "upcoming" ? "on" : ""} type="button" onClick={() => setView("upcoming")}>Upcoming</button>
          <button className={view === "all" ? "on" : ""} type="button" onClick={() => setView("all")}>All</button>
        </div>
      </div>

      <div className="card accent" style={{ padding: 28 }}>
        <div className="eyebrow">MONTHLY COMMITTED</div>
        <div className="figure money" style={{ fontSize: 52, lineHeight: 1, marginTop: 10 }}>{money(totalMonthlyCommitted, { currency: "USD" })}</div>
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
                <th>NAME</th>
                <th>FREQUENCY</th>
                <th>NEXT DATE</th>
                <th className="right">AMOUNT</th>
              </tr>
            </thead>
            <tbody>
              {groups.map((group) => (
                group.items.length > 0 ? [
                  <tr key={`${group.label}-hdr`}>
                    <td colSpan={4} style={{ paddingTop: 18, paddingBottom: 10, fontSize: 12, color: "var(--ink-faint)", fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.08em" }}>{group.label}</td>
                  </tr>,
                  ...group.items.map((item) => (
                    <tr key={`${group.label}-${item.merchantRaw}-${item.nextExpected}`}>
                      <td><div className="row row-sm"><span className="cswatch" style={{ background: item.categoryColor || (item.lastAmountCents > 0 ? "var(--accent)" : "var(--ink-faint)") }} /><div><div>{item.merchantRaw}</div><div className="muted" style={{ fontSize: 12 }}>{item.categoryLabel || group.label}</div></div></div></td>
                      <td><span className="chip">{recurringFrequency(item)}</span></td>
                      <td><span className="mono muted">{new Date(item.nextExpected).toLocaleDateString("en-US", { month: "short", day: "numeric" })}</span></td>
                      <td className="right"><span className={`money ${item.lastAmountCents > 0 ? "pos" : ""}`}>{money(item.lastAmountCents, { currency: "USD", decimals: 2 })}</span></td>
                    </tr>
                  )),
                ] : null
              ))}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  );
}
