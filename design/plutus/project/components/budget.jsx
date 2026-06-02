/* Budget — rethought from scratch.
   Glanceable cards, not a multi-month spreadsheet.
   Optimized for "where am I right now in this month?" */

function Budget() {
  const { categories, categoryGroups, budgetGrid, toBudget, incomeThisMonth, assignedThisMonth } = FS;
  const [mode, setMode] = React.useState("envelope");
  const [sort, setSort] = React.useState("group"); // group | stress | size | activity
  const [planOpen, setPlanOpen] = React.useState(false);

  // Compute envelope state for each category in May
  const envelopes = categories.map(c => {
    const may = budgetGrid[c.id]?.may || { b: 0, s: 0, c: 0 };
    const budget = may.b + may.c; // available
    const spent = may.s;
    const remaining = budget - spent;
    const pct = budget > 0 ? Math.min(100, (spent / budget) * 100) : 0;
    const over = spent > budget;
    const overBy = over ? spent - budget : 0;
    // Status
    let status;
    if (over)         status = { tone: "negative", label: `Over by $${overBy}`, severity: 3 };
    else if (pct > 90) status = { tone: "warning",  label: "Tight",              severity: 2 };
    else if (pct > 60) status = { tone: "neutral",  label: "On pace",            severity: 1 };
    else               status = { tone: "positive", label: "Plenty left",        severity: 0 };
    return { ...c, may, budget, spent, remaining, pct, over, overBy, status };
  });

  // Sort
  const sorted = [...envelopes].sort((a, b) => {
    if (sort === "stress")   return b.status.severity - a.status.severity || b.spent - a.spent;
    if (sort === "size")     return b.budget - a.budget;
    if (sort === "activity") return b.txns - a.txns;
    return 0; // group default, handled by grouping below
  });

  // Month progress
  const todayDay = FS.today.d;
  const totalDays = 31;
  const monthPct = (todayDay / totalDays) * 100;
  const totalSpent = envelopes.reduce((s, e) => s + e.spent, 0);
  const totalBudget = envelopes.reduce((s, e) => s + e.budget, 0);
  const projectedEom = Math.round((totalSpent / todayDay) * totalDays);
  const burnPerDay = totalSpent / todayDay;
  const daysLeft = totalDays - todayDay;
  const projectedRemainder = projectedEom - totalSpent;
  const onPaceText = projectedEom < totalBudget
    ? `On pace to end May at about $${projectedEom.toLocaleString()} — $${(totalBudget - projectedEom).toLocaleString()} under budget.`
    : `On pace to overspend by about $${(projectedEom - totalBudget).toLocaleString()}.`;

  // Group cards
  const grouped = sort === "group"
    ? categoryGroups.map(g => ({ ...g, items: sorted.filter(c => c.group === g.id) }))
    : [{ id: "all", label: "All envelopes", hint: "", items: sorted }];

  return (
    <div className="screen">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Budget · May 2026 · day {todayDay} of {totalDays}</div>
          <h1 className="h1" style={{ fontSize: 32, marginTop: 8 }}>Where the plan stands today.</h1>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <button className="btn primary" onClick={() => setPlanOpen(true)}>
            <I.Sparkle /> Plan next month
          </button>
          <div className="toolbar">
            <button className={mode === "envelope" ? "on" : ""} onClick={() => setMode("envelope")}>Envelope</button>
            <button className={mode === "tracking" ? "on" : ""} onClick={() => setMode("tracking")}>Tracking</button>
          </div>
        </div>
      </div>

      {/* Month progress hero */}
      <div className="card" style={{ marginTop: 16, padding: 28, background: "linear-gradient(135deg, var(--accent-2) 0%, var(--surface) 60%)", border: "1px solid var(--accent-3)" }}>
        <div className="budget-hero-row" style={{ display: "grid", gridTemplateColumns: "1.4fr 3fr", gap: 28, alignItems: "center" }}>
          <div>
            <div className="eyebrow" style={{ marginBottom: 10 }}>Month progress</div>
            <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
              <div className="figure" style={{ fontSize: 64, lineHeight: 1, color: "var(--accent)", letterSpacing: "-0.04em" }}>
                ${(totalBudget - totalSpent).toLocaleString()}
              </div>
              <span style={{ fontSize: 16, color: "var(--ink-mute)" }}>left to spend</span>
            </div>
            <div style={{ marginTop: 16, position: "relative", height: 10, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: monthPct + "%", background: "var(--ink-faint)", opacity: 0.4, borderRadius: 999 }} title="Time elapsed" />
              <div style={{ position: "absolute", left: 0, top: 0, bottom: 0, width: ((totalSpent/totalBudget)*100) + "%", background: "var(--accent)", borderRadius: 999, boxShadow: "0 0 12px var(--accent-3)" }} title="Spent" />
            </div>
            <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8, fontSize: 12, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
              <span>{Math.round(monthPct)}% through May</span>
              <span>{Math.round((totalSpent/totalBudget)*100)}% spent</span>
              <span>{daysLeft} days left</span>
            </div>
          </div>

          <div className="budget-hero-stats" style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 14 }}>
            <Stat label="Budgeted" value={<Currency value={totalBudget} />} sub={<span className="muted" style={{ fontSize: 12.5 }}>10 envelopes</span>} />
            <Stat label="Spent so far" value={<Currency value={totalSpent} />} sub={<span className="muted" style={{ fontSize: 12.5 }}>${Math.round(burnPerDay)}/day pace</span>} />
            <Stat label="Projected EOM" value={<Currency value={projectedEom} />} sub={
              projectedEom < totalBudget ?
                <span className="npill pos">−${(totalBudget - projectedEom).toLocaleString()} vs plan</span> :
                <span className="npill neg">+${(projectedEom - totalBudget).toLocaleString()} vs plan</span>
            } />
          </div>
        </div>
        <p style={{ marginTop: 22, fontSize: 14.5, color: "var(--ink-2)", lineHeight: 1.6, maxWidth: "72ch" }}>
          {onPaceText} <span className="muted">Three categories need a glance — they're listed below in the "needs attention" row.</span>
        </p>
      </div>

      {/* To Budget pill (envelope mode) */}
      {mode === "envelope" && (
        <div style={{ marginTop: 20, padding: "16px 22px", display: "flex", justifyContent: "space-between", alignItems: "center", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 12 }}>
          <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
            <div style={{ width: 36, height: 36, borderRadius: 10, background: toBudget >= 0 ? "var(--accent-2)" : "var(--negative-2)", color: toBudget >= 0 ? "var(--accent)" : "var(--negative)", display: "grid", placeItems: "center" }}>
              <I.Sparkle />
            </div>
            <div>
              <div className="eyebrow">To Budget · unassigned</div>
              <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
                <span className="figure" style={{ fontSize: 24, color: toBudget >= 0 ? "var(--accent)" : "var(--negative)" }}>${Math.abs(toBudget).toLocaleString()}</span>
                <span className="muted" style={{ fontSize: 13 }}>of ${incomeThisMonth.toLocaleString()} income · ${assignedThisMonth.toLocaleString()} assigned</span>
              </div>
            </div>
          </div>
          <div style={{ display: "flex", gap: 8 }}>
            <button className="btn outline sm" onClick={() => window.toast?.("Pick a goal", { sub: "House Fund · Italy · Emergency · Amex payoff" })}>Assign to a goal</button>
            <button className="btn sm" onClick={() => window.toast?.(`Parked $${Math.abs(toBudget).toLocaleString()} in House Fund`, { kind: "success", sub: "Re-balance available anytime", action: { label: "Undo", onClick: () => window.toast?.("Restored to unassigned") } })}>Park in House Fund</button>
          </div>
        </div>
      )}

      {/* Needs attention row */}
      <div className="section" style={{ marginTop: 36 }}>
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>Needs a glance · {envelopes.filter(e => e.status.severity >= 2).length}</div>
            <h2 className="h1" style={{ fontSize: 22 }}>Just these — the rest is fine.</h2>
          </div>
        </div>
        <div className="env-grid" style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14 }}>
          {envelopes.filter(e => e.status.severity >= 2).map(e => (
            <EnvelopeCard key={e.id} env={e} mode={mode} highlight />
          ))}
        </div>
      </div>

      {/* Sort + groups */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>All envelopes · {envelopes.length}</div>
            <h2 className="h1" style={{ fontSize: 22 }}>Each one, on its own.</h2>
          </div>
          <div className="toolbar">
            <button className={sort === "group" ? "on" : ""} onClick={() => setSort("group")}>By group</button>
            <button className={sort === "stress" ? "on" : ""} onClick={() => setSort("stress")}>By stress</button>
            <button className={sort === "size" ? "on" : ""} onClick={() => setSort("size")}>By size</button>
            <button className={sort === "activity" ? "on" : ""} onClick={() => setSort("activity")}>By activity</button>
          </div>
        </div>

        {grouped.map(g => (
          <div key={g.id} style={{ marginBottom: g.id === grouped[grouped.length - 1].id ? 0 : 32 }}>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 14 }}>
              <div style={{ display: "flex", alignItems: "baseline", gap: 12 }}>
                <span className="eyebrow">{g.label}</span>
                {g.hint && <span className="muted" style={{ fontSize: 12.5 }}>{g.hint}</span>}
              </div>
              {sort === "group" && (
                <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>
                  ${g.items.reduce((s, e) => s + e.spent, 0).toLocaleString()} / ${g.items.reduce((s, e) => s + e.budget, 0).toLocaleString()}
                </span>
              )}
            </div>
            <div className="env-grid" style={{ display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 14 }}>
              {g.items.map(e => <EnvelopeCard key={e.id} env={e} mode={mode} />)}
            </div>
          </div>
        ))}
      </div>

      {/* Quiet historical reference */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>Last 5 months · for context</div>
            <h2 className="h1" style={{ fontSize: 22 }}>How each envelope has run.</h2>
          </div>
          <button className="btn ghost sm" onClick={() => window.toast?.("Expanding history", { sub: "Loading 12 months · sit tight" })}>Show all <I.ArrowR /></button>
        </div>
        <HistoryStrip envelopes={envelopes} />
      </div>

      {planOpen && <PlanNextMonth onClose={() => setPlanOpen(false)} />}
    </div>
  );
}

function EnvelopeCard({ env, mode, highlight }) {
  const tone = env.status.tone;
  const barColor = tone === "negative" ? "var(--negative)" :
                   tone === "warning"  ? "var(--warning)"  :
                                          env.color;
  const cardStyle = {
    padding: 22,
    border: highlight && tone === "negative" ? "1px solid var(--negative)" :
            highlight && tone === "warning"  ? "1px solid var(--warning)" :
                                                "1px solid var(--line)",
    background: highlight && tone === "negative" ? "linear-gradient(135deg, var(--negative-2) 0%, var(--surface) 60%)" :
                highlight && tone === "warning"  ? "linear-gradient(135deg, var(--warning-2) 0%, var(--surface) 60%)" :
                                                    "var(--surface)",
  };

  return (
    <div className="card" style={cardStyle}>
      {/* Header */}
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 16 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <span style={{ width: 32, height: 32, borderRadius: 8, background: "var(--surface-2)", display: "grid", placeItems: "center", fontSize: 16 }}>{env.icon}</span>
          <div>
            <div style={{ fontSize: 15, fontWeight: 500, letterSpacing: "-0.005em" }}>{env.label}</div>
            <div className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>{env.txns} txn{env.txns === 1 ? "" : "s"} this month</div>
          </div>
        </div>
        <span className={`chip ${tone === "negative" ? "negative" : tone === "warning" ? "warning" : tone === "positive" ? "positive" : ""}`} style={{ padding: "3px 9px" }}>
          {env.status.label}
        </span>
      </div>

      {/* Big number */}
      <div style={{ marginBottom: 12 }}>
        <div className="figure" style={{ fontSize: 34, lineHeight: 1, letterSpacing: "-0.03em", color: env.over ? "var(--negative)" : "var(--ink)" }}>
          ${Math.abs(env.remaining).toLocaleString()}
        </div>
        <div className="muted" style={{ fontSize: 12.5, marginTop: 4 }}>
          {env.over ? "over budget" : "left to spend"}
        </div>
      </div>

      {/* Progress bar */}
      <div style={{ position: "relative", height: 8, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden", marginBottom: 10 }}>
        <div style={{
          position: "absolute", left: 0, top: 0, bottom: 0,
          width: Math.min(100, env.pct) + "%",
          background: barColor,
          borderRadius: 999,
          boxShadow: tone === "positive" || tone === "neutral" ? "0 0 8px " + barColor + "44" : "none",
        }} />
        {env.over && (
          <div style={{
            position: "absolute", right: 0, top: 0, bottom: 0,
            width: Math.min(20, ((env.overBy / env.budget) * 100)) + "%",
            background: "var(--negative)",
            borderTopRightRadius: 999,
            borderBottomRightRadius: 999,
            opacity: 0.7,
          }} />
        )}
      </div>

      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12.5, color: "var(--ink-mute)", fontFamily: "var(--mono)" }}>
        <span>spent ${env.spent.toLocaleString()}</span>
        <span>of ${env.budget.toLocaleString()}</span>
      </div>

      {/* Carry indicator */}
      {mode === "envelope" && env.may.c !== 0 && (
        <div style={{ marginTop: 12, paddingTop: 12, borderTop: "1px solid var(--hairline)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <span className="muted" style={{ fontSize: 12 }}>Carried from April</span>
          <span className={`num tabular`} style={{ fontSize: 12.5, color: env.may.c > 0 ? "var(--positive)" : "var(--negative)", fontFamily: "var(--mono)" }}>
            {env.may.c > 0 ? "+" : ""}${env.may.c}
          </span>
        </div>
      )}

      {/* Contextual action */}
      {tone === "negative" && (
        <button className="btn outline sm" style={{ width: "100%", justifyContent: "center", marginTop: 14 }} onClick={() => window.toast?.(`Pick an envelope to cover $${env.overBy}`, { sub: "Travel has $500 unspent—often the donor", kind: "warn" })}>
          Cover ${env.overBy} from another envelope
        </button>
      )}
      {tone === "warning" && (
        <div style={{ marginTop: 12, padding: "8px 12px", background: "var(--warning-2)", border: "1px solid var(--warning)", borderRadius: 7, fontSize: 12.5, color: "var(--ink-2)" }}>
          About <span className="strong">${Math.round(env.remaining / Math.max(1, (31 - FS.today.d)))} per day</span> left to stay under.
        </div>
      )}
    </div>
  );
}

function HistoryStrip({ envelopes }) {
  return (
    <div className="card flush">
      <div style={{ padding: "16px 22px", borderBottom: "1px solid var(--hairline)", display: "grid", gridTemplateColumns: "1.5fr repeat(5, 1fr)", gap: 14, alignItems: "center", fontSize: 12, fontFamily: "var(--mono)", color: "var(--ink-faint)", letterSpacing: "0.06em", textTransform: "uppercase" }}>
        <div>Category</div>
        <div style={{ textAlign: "right" }}>Mar</div>
        <div style={{ textAlign: "right" }}>Apr</div>
        <div style={{ textAlign: "right", color: "var(--accent)" }}>May · now</div>
        <div style={{ textAlign: "right" }}>Jun · planned</div>
        <div style={{ textAlign: "right" }}>Jul · planned</div>
      </div>
      {envelopes.map(e => {
        const monthIds = ["mar", "apr", "may", "jun", "jul"];
        const values = monthIds.map(m => FS.budgetGrid[e.id]?.[m] || { b: 0, s: 0 });
        const max = Math.max(...values.map(v => Math.max(v.b, v.s)));
        return (
          <div key={e.id} style={{ padding: "14px 22px", borderBottom: "1px solid var(--hairline)", display: "grid", gridTemplateColumns: "1.5fr repeat(5, 1fr)", gap: 14, alignItems: "center" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span className="cswatch" style={{ background: e.color }}></span>
              <span style={{ fontSize: 14 }}>{e.label}</span>
            </div>
            {values.map((v, i) => {
              const isCurrent = monthIds[i] === "may";
              const isFuture = i > 2;
              const value = isFuture ? v.b : v.s;
              const over = !isFuture && v.s > v.b;
              return (
                <div key={i} style={{ textAlign: "right" }}>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "flex-end", gap: 8 }}>
                    <div style={{ flex: 1, maxWidth: 70, height: 5, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                      <div style={{ width: `${(value / max) * 100}%`, height: "100%", background: over ? "var(--negative)" : isCurrent ? "var(--accent)" : e.color, opacity: isFuture ? 0.4 : 1 }} />
                    </div>
                    <span className="num tabular" style={{ fontSize: 13, color: over ? "var(--negative)" : isFuture ? "var(--ink-mute)" : isCurrent ? "var(--ink)" : "var(--ink-2)", minWidth: 50, fontFamily: "var(--mono)" }}>${value}</span>
                  </div>
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}

window.Budget = Budget;
