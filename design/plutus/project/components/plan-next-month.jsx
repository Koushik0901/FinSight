/* Plan Next Month — 5-question guided flow.
   Inspired by YNAB's method, rewritten in FinSight's voice. */

function PlanNextMonth({ onClose }) {
  const [step, setStep] = React.useState(0);
  const [buffer, setBuffer] = React.useState(800);
  const [sinkContributions, setSinkContributions] = React.useState(
    FS.sinkingFunds.reduce((a, s) => ({ ...a, [s.id]: Math.round((s.target - s.current) / 6) }), {})
  );
  const [goalContribs, setGoalContribs] = React.useState({
    g1: 1600, g2: 900, g3: 600, g4: 1209,
  });
  const [adjustments, setAdjustments] = React.useState({});

  // Steps definition
  const steps = [
    { id: "review", label: "Look back" },
    { id: "basics", label: "The basics" },
    { id: "notyet", label: "The not-yet" },
    { id: "buffer", label: "Breathing room" },
    { id: "pulls",  label: "The pulls" },
    { id: "adjust", label: "Adjust" },
    { id: "done",   label: "Done" },
  ];
  const cur = steps[step];
  const next = () => setStep(s => Math.min(steps.length - 1, s + 1));
  const back = () => setStep(s => Math.max(0, s - 1));

  // Compute running budget
  const fixedTotal = FS.categories.filter(c => c.group === "fixed").reduce((s, c) => s + (FS.budgetGrid[c.id]?.may?.b || 0), 0);
  const sinkTotal = Object.values(sinkContributions).reduce((s, v) => s + v, 0);
  const goalTotal = Object.values(goalContribs).reduce((s, v) => s + v, 0);
  const dailyTotal = FS.categories.filter(c => c.group === "daily" || c.group === "lifestyle" || c.group === "wellbeing").reduce((s, c) => s + (FS.budgetGrid[c.id]?.may?.b || 0), 0);
  const income = 10000;
  const planned = fixedTotal + sinkTotal + goalTotal + buffer + dailyTotal;
  const remaining = income - planned;

  return (
    <div className="onb-shell" style={{ background: "var(--bg)" }}>
      <div className="onb-top">
        <div className="brand" style={{ padding: 0 }}>
          <div className="mark"></div>
          <div className="wm">FinSight</div>
          <span className="muted" style={{ fontSize: 13, marginLeft: 14 }}>Plan next month · June 2026</span>
        </div>
        <div className="onb-steps">
          {steps.slice(0, -1).map((_, i) => (
            <div key={i} className={`onb-step-pip ${i < step ? "done" : ""} ${i === step ? "cur" : ""}`}></div>
          ))}
        </div>
        <button className="btn ghost sm" onClick={onClose}>Save & close</button>
      </div>

      <div className="onb-body">
        <div className="onb-left">
          <div className="num-step">Step {Math.min(step + 1, 6)} of 5 · {cur.label}</div>

          {cur.id === "review" && (
            <>
              <h1>First, look back.</h1>
              <p className="lead">
                Before deciding what June should be, a quick view of how May actually played out. The agent picks out the few things worth knowing — no shame, no celebration, just the facts.
              </p>
              <div style={{ display: "flex", flexDirection: "column", gap: 10, maxWidth: 460 }}>
                {[
                  { fact: "Dining ran $12 over budget.",          ctx: "First time this year. Mostly the Mosswood dinner." },
                  { fact: "Travel sat at $0 — fourth month in a row.", ctx: "$500 carried each month is now $1,994. Consider sweeping some toward the Italy goal." },
                  { fact: "Groceries closed $168 under.",         ctx: "Costco run replaced two grocery trips. Sustainable pattern." },
                  { fact: "PG&E was 2.1× normal.",                ctx: "Anomaly — worth a glance before locking June." },
                ].map((r, i) => (
                  <div key={i} className="card tight" style={{ padding: 14 }}>
                    <div className="strong" style={{ fontSize: 14 }}>{r.fact}</div>
                    <div className="muted" style={{ fontSize: 13, marginTop: 4, lineHeight: 1.5 }}>{r.ctx}</div>
                  </div>
                ))}
              </div>
            </>
          )}

          {cur.id === "basics" && (
            <>
              <h1>What's already <em>spoken for</em>?</h1>
              <p className="lead">
                Things that show up whether you plan for them or not. The agent pulled these from your detected recurring activity — confirm or adjust.
              </p>
              <div style={{ display: "flex", flexDirection: "column", gap: 6, maxWidth: 460 }}>
                {FS.categories.filter(c => c.group === "fixed").map(c => (
                  <div key={c.id} style={{ display: "grid", gridTemplateColumns: "auto 1fr auto", gap: 12, alignItems: "center", padding: "10px 14px", background: "var(--surface-2)", borderRadius: 8 }}>
                    <span style={{ fontSize: 16 }}>{c.icon}</span>
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 500 }}>{c.label}</div>
                      <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>3-month avg ${c.yearAvg}</div>
                    </div>
                    <div className="figure" style={{ fontSize: 16, color: "var(--accent)" }}>${FS.budgetGrid[c.id]?.may?.b || 0}</div>
                  </div>
                ))}
              </div>
              <div className="chip" style={{ marginTop: 18, alignSelf: "flex-start" }}><span className="dot"></span>Total fixed: ${fixedTotal}/mo</div>
            </>
          )}

          {cur.id === "notyet" && (
            <>
              <h1>What's coming that <em>isn't</em> monthly?</h1>
              <p className="lead">
                Insurance renewals, holidays, vet visits, the annual subscription you forgot about. Break each into monthly slivers so they don't ambush you.
              </p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {FS.sinkingFunds.map(s => {
                  const monthsLeft = 6;
                  return (
                    <div key={s.id} style={{ padding: 14, background: "var(--surface-2)", borderRadius: 8 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                        <div>
                          <div style={{ fontSize: 14, fontWeight: 500 }}>{s.name}</div>
                          <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>due {s.due} · ${s.current} of ${s.target}</div>
                        </div>
                        <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>${sinkContributions[s.id]}<span style={{ fontSize: 13, color: "var(--ink-mute)", marginLeft: 4 }}>/mo</span></div>
                      </div>
                      <input type="range" min="0" max="500" step="10" value={sinkContributions[s.id]}
                        onChange={e => setSinkContributions({ ...sinkContributions, [s.id]: parseInt(e.target.value) })}
                        style={{ width: "100%", marginTop: 10, accentColor: "var(--accent)" }} />
                    </div>
                  );
                })}
              </div>
              <div className="chip" style={{ marginTop: 18, alignSelf: "flex-start" }}><span className="dot"></span>Total set aside: ${sinkTotal}/mo</div>
            </>
          )}

          {cur.id === "buffer" && (
            <>
              <h1>How much should June already have, <em>today</em>?</h1>
              <p className="lead">
                Carry a buffer in and bills stop being events. Most people overthink this — somewhere between one week and one month of typical spend is plenty.
              </p>
              <div style={{ maxWidth: 460 }}>
                <div style={{ padding: 18, background: "var(--surface-2)", borderRadius: 10, marginTop: 8 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                    <span style={{ fontSize: 14 }}>Carry-over buffer</span>
                    <span className="figure" style={{ fontSize: 26, color: "var(--accent)" }}>${buffer.toLocaleString()}</span>
                  </div>
                  <input type="range" min="0" max="3500" step="50" value={buffer}
                    onChange={e => setBuffer(parseInt(e.target.value))}
                    style={{ width: "100%", marginTop: 12, accentColor: "var(--accent)" }} />
                  <div style={{ display: "flex", justifyContent: "space-between", marginTop: 6, fontSize: 12, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>
                    <span>$0 · live paycheck to paycheck</span>
                    <span>$3,500 · one month covered</span>
                  </div>
                </div>
                <div className="muted" style={{ fontSize: 13.5, marginTop: 14, lineHeight: 1.55 }}>
                  At <span className="accent-text strong">${buffer}</span>, you'll start June with about <span className="strong">{Math.round(buffer / 130)} days of typical spend</span> already covered. Bills land softly.
                </div>
              </div>
            </>
          )}

          {cur.id === "pulls" && (
            <>
              <h1>What are we <em>moving toward</em>?</h1>
              <p className="lead">
                Goals are the gravity. Each line below is one — tune what you'll contribute this month. Pause anything that should wait.
              </p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {FS.goals.filter(g => g.type !== "spending-cap").map(g => (
                  <div key={g.id} style={{ padding: 14, background: "var(--surface-2)", borderRadius: 8 }}>
                    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
                      <div>
                        <div style={{ fontSize: 14, fontWeight: 500 }}>{g.name}</div>
                        <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>{g.current.toLocaleString()} of {g.target.toLocaleString()} · ETA {g.eta}</div>
                      </div>
                      <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>${goalContribs[g.id] || 0}<span style={{ fontSize: 13, color: "var(--ink-mute)", marginLeft: 4 }}>/mo</span></div>
                    </div>
                    <input type="range" min="0" max="2000" step="50" value={goalContribs[g.id] || 0}
                      onChange={e => setGoalContribs({ ...goalContribs, [g.id]: parseInt(e.target.value) })}
                      style={{ width: "100%", marginTop: 10, accentColor: "var(--accent)" }} />
                  </div>
                ))}
              </div>
              <div className="chip" style={{ marginTop: 18, alignSelf: "flex-start" }}><span className="dot"></span>Toward goals: ${goalTotal}/mo</div>
            </>
          )}

          {cur.id === "adjust" && (
            <>
              <h1>What needs to <em>shift</em>?</h1>
              <p className="lead">
                Change anything else without guilt — life moves, the plan moves with it. The agent suggests adjustments based on your last 3 months.
              </p>
              <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
                {[
                  { cat: "dining",    label: "Lift Dining cap to $450",    why: "Hit the $400 cap 3 of last 4 months — $450 reflects reality." },
                  { cat: "travel",    label: "Raise Travel to $800 in Jul", why: "Italy trip planning. Build runway across June and July." },
                  { cat: "groceries", label: "Lower Groceries to $750",     why: "You've averaged $680 — leaves room without being tight." },
                ].map((r, i) => {
                  const c = FS.categories.find(x => x.id === r.cat);
                  const on = !!adjustments[r.cat];
                  return (
                    <div key={i} onClick={() => setAdjustments({ ...adjustments, [r.cat]: !on })}
                      style={{ padding: 14, background: on ? "var(--accent-2)" : "var(--surface-2)", border: "1px solid " + (on ? "var(--accent-3)" : "var(--line)"), borderRadius: 8, cursor: "pointer" }}>
                      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                          <span className="cswatch" style={{ background: c?.color }}></span>
                          <span style={{ fontSize: 14, fontWeight: 500 }}>{r.label}</span>
                        </div>
                        <span className={`tog ${on ? "on" : ""}`}></span>
                      </div>
                      <div className="muted" style={{ fontSize: 13, marginTop: 6, lineHeight: 1.5 }}>{r.why}</div>
                    </div>
                  );
                })}
              </div>
            </>
          )}

          {cur.id === "done" && (
            <>
              <h1>Your June plan is <em>ready</em>.</h1>
              <p className="lead">
                Every dollar of your $10,000 income has a job for June. You can change any of this later — the plan is a starting point, not a contract.
              </p>
              <div className="card" style={{ padding: 22, maxWidth: 460, marginTop: 8 }}>
                <div className="eyebrow" style={{ marginBottom: 12 }}>June 2026 · summary</div>
                <SummaryRow label="Fixed costs" value={fixedTotal} />
                <SummaryRow label="Set-asides" value={sinkTotal} />
                <SummaryRow label="Buffer carried in" value={buffer} />
                <SummaryRow label="Toward goals" value={goalTotal} />
                <SummaryRow label="Daily life" value={dailyTotal} />
                <div style={{ height: 1, background: "var(--line)", margin: "10px 0" }}></div>
                <SummaryRow label="Income" value={income} accent />
                <SummaryRow label="To budget" value={remaining} pillNegative={remaining < 0} pill={remaining >= 0} />
              </div>
              <div style={{ marginTop: 22, display: "flex", gap: 10 }}>
                <button className="btn primary" onClick={onClose}>Apply to June</button>
                <button className="btn outline" onClick={() => setStep(0)}>Start over</button>
              </div>
            </>
          )}
        </div>

        <div className="onb-right">
          <PlanPreview
            income={income}
            fixed={fixedTotal}
            sinks={sinkTotal}
            buffer={buffer}
            goals={goalTotal}
            daily={dailyTotal}
            remaining={remaining}
            step={step}
          />
        </div>
      </div>

      {cur.id !== "done" && (
        <div className="onb-foot">
          <button className="btn ghost" onClick={back} disabled={step === 0} style={{ opacity: step === 0 ? 0.4 : 1 }}>
            <I.ArrowL /> Back
          </button>
          <div className="muted" style={{ fontSize: 13, fontFamily: "var(--mono)" }}>
            {remaining >= 0
              ? `$${remaining.toLocaleString()} unassigned`
              : <span style={{ color: "var(--negative)" }}>$${Math.abs(remaining).toLocaleString()} over</span>}
          </div>
          <button className="btn primary" onClick={next}>
            Continue <I.ArrowR />
          </button>
        </div>
      )}
    </div>
  );
}

function SummaryRow({ label, value, accent, pill, pillNegative }) {
  return (
    <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", padding: "5px 0" }}>
      <span style={{ fontSize: 14, color: accent ? "var(--ink)" : "var(--ink-mute)", fontWeight: accent ? 500 : 400 }}>{label}</span>
      <span className={pillNegative ? "npill neg" : pill ? "npill pos" : "num tabular"} style={{ fontSize: 14, color: accent ? "var(--accent)" : pill || pillNegative ? undefined : "var(--ink)", fontFamily: "var(--sans)", fontWeight: accent ? 600 : 500, fontVariantNumeric: "tabular-nums" }}>
        ${Math.abs(value).toLocaleString()}
      </span>
    </div>
  );
}

function PlanPreview({ income, fixed, sinks, buffer, goals, daily, remaining, step }) {
  const segments = [
    { key: "fixed",  label: "Fixed",      value: fixed,  color: "var(--c-housing)",   active: step >= 1 },
    { key: "sinks",  label: "Set-asides", value: sinks,  color: "var(--c-utilities)", active: step >= 2 },
    { key: "buffer", label: "Buffer",     value: buffer, color: "var(--c-transport)", active: step >= 3 },
    { key: "goals",  label: "Goals",      value: goals,  color: "var(--accent)",      active: step >= 4 },
    { key: "daily",  label: "Daily life", value: daily,  color: "var(--c-groceries)", active: step >= 5 },
  ];
  return (
    <div style={{ width: "100%", maxWidth: 480 }}>
      <div className="eyebrow" style={{ marginBottom: 14 }}><span className="dot"></span>Live preview · June</div>
      <div className="card" style={{ padding: 22 }}>
        <div className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.06em", marginBottom: 8 }}>Income</div>
        <div className="figure" style={{ fontSize: 36, lineHeight: 1, marginBottom: 16 }}>${income.toLocaleString()}</div>

        {/* Stacked bar */}
        <div style={{ height: 24, borderRadius: 6, background: "var(--surface-2)", overflow: "hidden", display: "flex", gap: 2 }}>
          {segments.map(s => s.active && s.value > 0 && (
            <span key={s.key} title={`${s.label} $${s.value}`} style={{ width: `${(s.value / income) * 100}%`, background: s.color, transition: "width 0.3s" }} />
          ))}
          {remaining > 0 && (
            <span title={`Unassigned $${remaining}`} style={{ flex: 1, background: "var(--surface)", borderLeft: "1px dashed var(--ink-faint)", transition: "width 0.3s" }} />
          )}
        </div>

        {/* Legend */}
        <div style={{ marginTop: 18, display: "flex", flexDirection: "column", gap: 8 }}>
          {segments.map(s => (
            <div key={s.key} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", opacity: s.active ? 1 : 0.32, transition: "opacity 0.2s" }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span className="cswatch" style={{ background: s.color }}></span>
                <span style={{ fontSize: 14 }}>{s.label}</span>
              </div>
              <span className="num tabular" style={{ fontSize: 14 }}>${s.value.toLocaleString()}</span>
            </div>
          ))}
          <div style={{ height: 1, background: "var(--hairline)", margin: "4px 0" }}></div>
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
            <span style={{ fontSize: 14, fontWeight: 500 }}>{remaining >= 0 ? "Unassigned" : "Over"}</span>
            <span className={remaining >= 0 ? "num tabular accent-text" : "num tabular"} style={{ fontSize: 14, fontWeight: 600, color: remaining < 0 ? "var(--negative)" : undefined }}>
              ${Math.abs(remaining).toLocaleString()}
            </span>
          </div>
        </div>
      </div>

      {/* Method side-note */}
      <div style={{ marginTop: 18, padding: 14, border: "1px dashed var(--line-2)", borderRadius: 10 }}>
        <div className="eyebrow" style={{ marginBottom: 6 }}>The method · 5 questions</div>
        <ol style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 6 }}>
          {[
            "Look back at what actually happened",
            "What's already spoken for",
            "What's coming that isn't monthly",
            "How much should next month already have",
            "What are we moving toward",
            "What needs to shift",
          ].map((q, i) => (
            <li key={i} style={{ display: "flex", alignItems: "center", gap: 8, fontSize: 13, color: i === step ? "var(--ink)" : i < step ? "var(--ink-mute)" : "var(--ink-faint)" }}>
              <span style={{ width: 14, height: 14, borderRadius: 999, border: "1.5px solid " + (i <= step ? "var(--accent)" : "var(--line-2)"), background: i < step ? "var(--accent)" : "transparent", display: "grid", placeItems: "center", flexShrink: 0 }}>
                {i < step && <I.Check width="8" height="8" style={{ color: "var(--accent-ink)" }} />}
              </span>
              {q}
            </li>
          ))}
        </ol>
      </div>
    </div>
  );
}

window.PlanNextMonth = PlanNextMonth;
