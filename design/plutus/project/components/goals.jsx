function Goals() {
  const goals = FS.goals;
  const [typeFilter, setTypeFilter] = React.useState("all");
  const [scenarioGoal, setScenarioGoal] = React.useState(goals[0].id);
  const [extra, setExtra] = React.useState(0);

  const sel = goals.find(g => g.id === scenarioGoal);
  const baseMonths = Math.ceil((sel.target - sel.current) / sel.monthly);
  const newMonths = Math.ceil((sel.target - sel.current) / (sel.monthly + extra));
  const monthsSaved = Math.max(0, baseMonths - newMonths);

  const typeCounts = goals.reduce((m, g) => ({ ...m, [g.type]: (m[g.type] || 0) + 1 }), {});
  const visibleGoals = typeFilter === "all" ? goals : goals.filter(g => g.type === typeFilter);

  return (
    <div className="screen">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Goals · 5 active</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Things you're moving toward.</h1>
        </div>
        <button className="btn" onClick={() => window.toast?.("New goal form", { sub: "Pick a type · set a target · commit a monthly", kind: "accent" })}><I.Plus /> New goal</button>
      </div>

      <p className="muted" style={{ maxWidth: 660, fontSize: 14, lineHeight: 1.6, marginTop: 10 }}>
        A goal is a horizon line on your future runway. The agent moves money toward each on the cadence you set, and shows you when reality drifts from the plan.
      </p>

      {/* Goal types tabs */}
      <div className="toolbar" style={{ marginTop: 20, display: "inline-flex" }}>
        <button className={typeFilter === "all" ? "on" : ""} onClick={() => setTypeFilter("all")}>All <span style={{ color: "var(--ink-faint)", marginLeft: 4 }}>{goals.length}</span></button>
        <button className={typeFilter === "save-by-date" ? "on" : ""} onClick={() => setTypeFilter("save-by-date")}>Save by date <span style={{ color: "var(--ink-faint)", marginLeft: 4 }}>{typeCounts["save-by-date"] || 0}</span></button>
        <button className={typeFilter === "build-balance" ? "on" : ""} onClick={() => setTypeFilter("build-balance")}>Build balance <span style={{ color: "var(--ink-faint)", marginLeft: 4 }}>{typeCounts["build-balance"] || 0}</span></button>
        <button className={typeFilter === "debt-payoff" ? "on" : ""} onClick={() => setTypeFilter("debt-payoff")}>Debt payoff <span style={{ color: "var(--ink-faint)", marginLeft: 4 }}>{typeCounts["debt-payoff"] || 0}</span></button>
        <button className={typeFilter === "spending-cap" ? "on" : ""} onClick={() => setTypeFilter("spending-cap")}>Spending cap <span style={{ color: "var(--ink-faint)", marginLeft: 4 }}>{typeCounts["spending-cap"] || 0}</span></button>
      </div>

      <div style={{ display: "flex", flexDirection: "column", gap: 10, marginTop: 16 }}>
        {visibleGoals.map(g => <GoalCard key={g.id} g={g} />)}
        {!visibleGoals.length && <div className="empty-dash" style={{ marginTop: 8 }}><div className="h1" style={{ fontSize: 22 }}>No goals here.</div><div className="muted" style={{ fontSize: 13, marginTop: 8 }}>Switch the filter or create one.</div></div>}
      </div>

      {/* What-if scenario */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>What if · scenario</div>
            <h2 className="h1" style={{ fontSize: 22 }}>Move a slider, see the future shift.</h2>
          </div>
        </div>

        <div className="card" style={{ padding: 26 }}>
          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 32 }}>
            <div>
              <div className="eyebrow" style={{ marginBottom: 10 }}>Goal</div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {goals.filter(g => g.type !== "spending-cap").map(g => (
                  <button key={g.id} onClick={() => setScenarioGoal(g.id)}
                    style={{
                      display: "flex", justifyContent: "space-between", alignItems: "center",
                      padding: "10px 12px", borderRadius: 8,
                      background: scenarioGoal === g.id ? "var(--surface-2)" : "transparent",
                      border: "1px solid " + (scenarioGoal === g.id ? "var(--line-2)" : "transparent"),
                      cursor: "pointer", textAlign: "left",
                    }}>
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 500 }}>{g.name}</div>
                      <div className="muted" style={{ fontSize: 12.5, marginTop: 2 }}>{g.owner} · ETA {g.eta}</div>
                    </div>
                    {scenarioGoal === g.id && <I.Check style={{ color: "var(--accent)" }} />}
                  </button>
                ))}
              </div>

              <div style={{ marginTop: 22 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 10 }}>
                  <span className="eyebrow">Extra per month</span>
                  <span className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>+${extra.toLocaleString()}</span>
                </div>
                <input type="range" min="0" max="1500" step="50" value={extra} onChange={e => setExtra(parseInt(e.target.value))}
                  style={{ width: "100%", accentColor: "var(--accent)" }} />
                <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 12, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
                  <span>$0</span>
                  <span>$750</span>
                  <span>$1,500</span>
                </div>
              </div>
            </div>

            <div style={{ padding: 22, background: "linear-gradient(180deg, var(--accent-2) 0%, var(--surface-2) 60%)", borderRadius: 12, border: "1px solid var(--accent-3)" }}>
              <div className="eyebrow" style={{ marginBottom: 14 }}>Updated horizon</div>
              <div style={{ display: "flex", alignItems: "baseline", gap: 10 }}>
                <span className="figure" style={{ fontSize: 56, lineHeight: 1, color: "var(--accent)" }}>{newMonths}</span>
                <span className="muted" style={{ fontSize: 16 }}>months to go</span>
              </div>
              <div style={{ marginTop: 16, fontSize: 14, lineHeight: 1.55, color: "var(--ink-2)" }}>
                {extra === 0 ? (
                  <span>You're on track for the original plan. Drag the slider to see what changes.</span>
                ) : (
                  <span>
                    Adding <span className="strong accent-text">${extra}/mo</span> brings <span className="strong">{sel.name}</span> in by <span className="strong accent-text">{monthsSaved} {monthsSaved === 1 ? "month" : "months"}</span> — moving the ETA from <span className="strong">{sel.eta}</span> to roughly <span className="strong">{shiftMonth(sel.eta, -monthsSaved)}</span>.
                  </span>
                )}
              </div>
              <div style={{ marginTop: 20, display: "flex", gap: 8 }}>
                <button className="btn primary" disabled={extra === 0} style={{ opacity: extra === 0 ? 0.5 : 1 }} onClick={() => {
                  if (extra === 0) return;
                  window.toast?.(`Applied +$${extra}/mo to ${sel.name}`, { kind: "success", sub: `ETA: ${shiftMonth(sel.eta, -monthsSaved)} · saves ${monthsSaved} mo`, action: { label: "Undo", onClick: () => { setExtra(0); window.toast?.("Reverted"); } } });
                  setExtra(0);
                }}>Apply this scenario</button>
                <button className="btn ghost" onClick={() => setExtra(0)}>Reset</button>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Sinking funds */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>Sinking funds · {FS.sinkingFunds.length}</div>
            <h2 className="h1" style={{ fontSize: 22 }}>Quietly set aside for the inevitable.</h2>
          </div>
          <button className="btn outline sm" onClick={() => window.toast?.("New sinking-fund form", { sub: "Set a name, due date, and target" })}><I.Plus /> New bucket</button>
        </div>

        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 12 }}>
          {FS.sinkingFunds.map(s => {
            const pct = (s.current / s.target) * 100;
            return (
              <div key={s.id} className="card tight" style={{ padding: 16 }}>
                <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                  <div>
                    <div className="h3">{s.name}</div>
                    <div className="muted" style={{ fontSize: 12.5, marginTop: 2 }}>Due {s.due}</div>
                  </div>
                  <div className="figure" style={{ fontSize: 18 }}>{Math.round(pct)}%</div>
                </div>
                <div className="goal-bar" style={{ marginTop: 12, height: 5 }}><span style={{ width: pct + "%" }}></span></div>
                <div style={{ display: "flex", justifyContent: "space-between", marginTop: 8, fontSize: 12.5, color: "var(--ink-mute)", fontFamily: "var(--mono)" }}>
                  <span>${s.current} of ${s.target}</span>
                  <span>+${Math.round((s.target - s.current) / 6)}/mo planned</span>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      {/* Horizon */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>Horizon</div>
            <h2 className="h1" style={{ fontSize: 22 }}>When each goal lands.</h2>
          </div>
        </div>
        <GoalHorizon goals={goals.filter(g => g.eta !== "—")} />
      </div>
    </div>
  );
}

function shiftMonth(label, months) {
  // "Mar 2027" + months → label
  const [m, y] = label.split(" ");
  const list = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
  let mi = list.indexOf(m);
  let yi = parseInt(y);
  mi += months;
  while (mi < 0) { mi += 12; yi--; }
  while (mi >= 12) { mi -= 12; yi++; }
  return `${list[mi]} ${yi}`;
}

function GoalCard({ g }) {
  const pct = g.type === "spending-cap" ? (g.current / g.target) * 100 : (g.current / g.target) * 100;
  const paceColor = g.pace === "needs attention" ? "negative" : g.pace === "ahead" ? "positive" : "accent";
  const typeLabel = {
    "save-by-date": "Save by date",
    "build-balance": "Build balance",
    "debt-payoff": "Pay off debt",
    "spending-cap": "Spending cap",
  }[g.type] || g.type;
  const over = g.type === "spending-cap" && g.current > g.target;
  return (
    <div className="card">
      <div style={{ display: "grid", gridTemplateColumns: "1.5fr 1fr 1fr", gap: 28, alignItems: "center" }}>
        <div>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span className="chip">{typeLabel}</span>
            <span className="chip">{g.owner}</span>
            <span className={`chip ${paceColor}`}><span className="dot"></span>{g.pace}</span>
          </div>
          <div className="h1" style={{ fontSize: 22, marginTop: 8 }}>{g.name}</div>
          <div className="muted" style={{ fontSize: 13, marginTop: 4 }}>
            {g.type === "debt-payoff" ? `Paying ${FS.fmt(g.monthly)}/mo · clears ${g.eta}` :
             g.type === "spending-cap" ? `${FS.fmt(g.target)}/mo cap · this month at ${FS.fmt(g.current)}` :
             `Auto-moves ${FS.fmt(g.monthly)}/mo · ETA ${g.eta}`}
          </div>
        </div>

        <div>
          <div className="eyebrow" style={{ marginBottom: 6 }}>{g.type === "spending-cap" ? "This month" : "Progress"}</div>
          <div className="goal-bar" style={{ height: 7 }}>
            <span style={{ width: Math.min(100, pct) + "%", background: over ? "var(--negative)" : "var(--accent)" }}></span>
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6 }}>
            <span className="num tabular" style={{ fontSize: 13 }}>${g.current.toLocaleString()}</span>
            <span className="num tabular muted" style={{ fontSize: 13 }}>of ${g.target.toLocaleString()}</span>
          </div>
        </div>

        <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", gap: 8 }}>
          <div className="figure" style={{ fontSize: 32, lineHeight: 1, color: over ? "var(--negative)" : "var(--ink)" }}>
            {Math.round(pct)}<span className="muted" style={{ fontSize: 14 }}>%</span>
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button className="btn ghost sm" onClick={(e) => { e.stopPropagation(); window.toast?.(`Paused \u201c${g.name}\u201d`, { kind: "warn", sub: "Auto-transfers will stop", action: { label: "Resume", onClick: () => window.toast?.("Resumed") } }); }}>Pause</button>
            <button className="btn outline sm" onClick={(e) => { e.stopPropagation(); window.toast?.(`Editing \u201c${g.name}\u201d`, { sub: "Adjust target, ETA, or monthly" }); }}>Adjust</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function GoalHorizon({ goals }) {
  const monthsFromNow = (label) => {
    const [m, y] = label.split(" ");
    const mi = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"].indexOf(m);
    return (parseInt(y) - 2026) * 12 + (mi - 4);
  };
  const maxM = 14;
  return (
    <div className="card" style={{ padding: "22px 26px 14px" }}>
      <div style={{ position: "relative", height: 40 + goals.length * 44, paddingTop: 26 }}>
        <div style={{ position: "absolute", top: 0, left: 0, right: 0, height: 22, display: "flex", justifyContent: "space-between" }}>
          {["May","Jul","Sep","Nov","Jan '27","Mar","Jun"].map((m, i) => (
            <div key={i} style={{ textAlign: "center", flex: 1 }}>
              <div style={{ fontFamily: "var(--mono)", fontSize: 11.5, color: "var(--ink-faint)" }}>{m}</div>
              <div style={{ width: 1, height: 6, background: "var(--hairline)", margin: "4px auto 0" }} />
            </div>
          ))}
        </div>
        <div style={{ position: "absolute", top: 22, left: 0, bottom: 0, width: 1, background: "var(--accent)", opacity: 0.6, boxShadow: "0 0 8px var(--accent)" }}></div>

        {goals.map((g, i) => {
          const m = monthsFromNow(g.eta);
          const left = Math.min(100, Math.max(0, (m / maxM) * 100));
          return (
            <div key={g.id} style={{ position: "absolute", left: 0, right: 0, top: 26 + i * 44, height: 36 }}>
              <div style={{ position: "absolute", top: "50%", left: 0, width: left + "%", height: 1, background: "var(--hairline)" }} />
              <div style={{ position: "absolute", top: "calc(50% - 1.5px)", left: 0, width: ((g.current/g.target) * left) + "%", height: 3, background: g.pace === "needs attention" ? "var(--negative)" : "var(--accent)", borderRadius: 999, boxShadow: g.pace !== "needs attention" ? "0 0 6px var(--accent-3)" : "" }} />
              <div style={{ position: "absolute", left: `calc(${left}% - 7px)`, top: "calc(50% - 7px)", width: 14, height: 14, borderRadius: 999, background: "var(--surface)", border: "2px solid var(--accent)", display: "grid", placeItems: "center" }}>
                <span style={{ width: 4, height: 4, borderRadius: 999, background: "var(--accent)" }}></span>
              </div>
              <div style={{ position: "absolute", left: `calc(${left}% + 14px)`, top: 0, height: "100%", display: "flex", flexDirection: "column", justifyContent: "center" }}>
                <div style={{ fontSize: 13.5, fontWeight: 500, whiteSpace: "nowrap" }}>{g.name}</div>
                <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>{g.eta} · <Currency value={g.target} /></div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

window.Goals = Goals;
