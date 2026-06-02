/* Scenarios — natural language what-if planner. */

function Scenarios() {
  const [query, setQuery] = React.useState("");
  const [active, setActive] = React.useState(FS.scenarios[0]);
  const [thinking, setThinking] = React.useState(false);
  const [thinkingFor, setThinkingFor] = React.useState(null);
  const [history, setHistory] = React.useState([
    { id: "h1", title: "What if we lock in the mortgage refinance now?", when: "2 days ago", outcome: "Saves $4,200 over remaining term. Worth it.", scenario: FS.scenarios[1] },
    { id: "h2", title: "What if we delay Italy by 6 months?",            when: "1 week ago",  outcome: "Goal pace eases by 33%; flight prices uncertain.", scenario: FS.scenarios[3] },
    { id: "h3", title: "What if I drop Adam's gym?",                     when: "2 weeks ago", outcome: "Save $1,788/yr but you average 9 visits/mo.", scenario: FS.scenarios[4] },
  ]);

  const run = (s) => {
    setActive(s);
    setThinkingFor(s.id);
    setThinking(true);
    setTimeout(() => setThinking(false), 1200);
  };

  const submitQuery = (e) => {
    e?.preventDefault?.();
    if (!query.trim()) return;
    // Match to existing scenario or invent one
    const found = FS.scenarios.find(s => s.title.toLowerCase().includes(query.toLowerCase().split(" ")[0]));
    if (found) run(found);
    else run({ id: "custom", title: query, impact: { runway: -45, goalsSlip: ["House Fund: +2 mo"], coverable: true } });
  };

  return (
    <div className="screen">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Scenarios · run any what-if</div>
          <h1 className="h1" style={{ fontSize: 32, marginTop: 8 }}>Imagine a future, see the math.</h1>
        </div>
      </div>

      <p className="muted" style={{ maxWidth: "72ch", fontSize: 15, lineHeight: 1.6, marginTop: 12 }}>
        Ask the agent any "what if" question. It re-runs your forecasts against current pace, goals, and pending bills — and tells you whether the move is coverable, which goals it touches, and what to watch.
      </p>

      {/* Big ask input */}
      <form onSubmit={submitQuery} style={{ marginTop: 22 }}>
        <div style={{
          display: "flex", alignItems: "center", gap: 14,
          padding: "20px 24px",
          background: "var(--surface)",
          border: "1px solid var(--line-2)",
          borderRadius: 14,
        }}>
          <I.Sparkle style={{ color: "var(--accent)" }} />
          <input
            type="text" value={query} onChange={e => setQuery(e.target.value)}
            placeholder="What if I take a 6-month sabbatical starting October?"
            style={{ flex: 1, background: "transparent", border: 0, outline: 0, fontSize: 18, color: "var(--ink)", fontFamily: "var(--sans)", letterSpacing: "-0.015em" }}
          />
          <button type="submit" className="btn primary"><I.ArrowR /> Run</button>
        </div>
      </form>

      {/* Suggested scenarios */}
      <div style={{ marginTop: 22 }}>
        <div className="eyebrow" style={{ marginBottom: 12 }}>Or pick a starting point</div>
        <div style={{ display: "flex", gap: 8, flexWrap: "wrap" }}>
          {FS.scenarios.map(s => (
            <button key={s.id} onClick={() => run(s)}
              style={{
                display: "inline-flex", alignItems: "center", gap: 8,
                padding: "8px 14px",
                borderRadius: 999,
                fontSize: 13,
                background: active.id === s.id ? "var(--accent-2)" : "var(--surface)",
                border: "1px solid " + (active.id === s.id ? "var(--accent-3)" : "var(--line)"),
                color: active.id === s.id ? "var(--accent)" : "var(--ink-2)",
                cursor: "pointer",
                fontWeight: 500,
                letterSpacing: "-0.005em",
              }}>
              {s.title}
            </button>
          ))}
        </div>
      </div>

      {/* Active scenario */}
      {active && <ScenarioResult scenario={active} thinking={thinking} onSave={(s) => {
        setHistory(h => [{ id: "h_" + Math.random().toString(36).slice(2, 6), title: s.title, when: "just now", outcome: "Saved · ·", scenario: s }, ...h]);
        window.toast?.("Scenario saved", { kind: "success", sub: s.title, action: { label: "Open", onClick: () => {} } });
      }} />}

      {/* History */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>Recent scenarios you've run</div>
            <h2 className="h1" style={{ fontSize: 22 }}>Your sandbox.</h2>
          </div>
          <button className="btn ghost sm" onClick={() => {
            if (!history.length) return;
            if (confirm(`Clear ${history.length} scenarios from history?`)) {
              setHistory([]);
              window.toast?.("History cleared", { kind: "warn" });
            }
          }}>Clear</button>
        </div>
        <div className="card flush">
          {!history.length && (
            <div style={{ padding: 32, textAlign: "center", color: "var(--ink-faint)", fontSize: 13 }}>
              No scenarios saved. Run one above to keep it here.
            </div>
          )}
          {history.map((h, i) => (
            <div key={h.id} style={{ display: "grid", gridTemplateColumns: "1fr auto auto", gap: 18, padding: "16px 22px", borderBottom: i === history.length - 1 ? "0" : "1px solid var(--hairline)", alignItems: "center" }}>
              <div>
                <div style={{ fontSize: 14, color: "var(--ink)" }}>{h.title}</div>
                <div className="muted" style={{ fontSize: 12.5, marginTop: 4 }}>{h.outcome}</div>
              </div>
              <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>{h.when}</span>
              <button className="btn ghost sm" onClick={() => h.scenario && run(h.scenario)}>Re-run</button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function ScenarioResult({ scenario, thinking, onSave }) {
  const [forecastRange, setForecastRange] = React.useState("12M");
  if (thinking) {
    return (
      <div className="card" style={{ marginTop: 24, padding: 36, textAlign: "center" }}>
        <div style={{ display: "inline-flex", alignItems: "center", gap: 14 }}>
          <span style={{ width: 16, height: 16, borderRadius: 999, background: "var(--accent)", animation: "pulse 1.4s infinite" }}></span>
          <span style={{ fontSize: 15, color: "var(--ink-2)" }}>Running your scenario across 14 months of data…</span>
        </div>
        <div className="muted" style={{ fontSize: 12.5, marginTop: 16, fontFamily: "var(--mono)" }}>
          Modeling cash flow · checking goal pace · projecting buffers
        </div>
      </div>
    );
  }

  const { impact } = scenario;
  const coverable = impact.coverable;

  return (
    <div className="section">
      <div className="card" style={{
        padding: 0,
        border: "1px solid " + (coverable ? "var(--accent-3)" : "var(--negative)"),
        background: coverable
          ? "linear-gradient(135deg, var(--accent-2) 0%, var(--surface) 60%)"
          : "linear-gradient(135deg, var(--negative-2) 0%, var(--surface) 60%)",
        overflow: "hidden",
      }}>
        <div style={{ padding: 28 }}>
          <div className="eyebrow" style={{ marginBottom: 12 }}>
            <span className="dot" style={{ background: coverable ? "var(--accent)" : "var(--negative)" }}></span>
            Verdict
          </div>
          <div style={{ fontSize: 28, fontWeight: 500, letterSpacing: "-0.025em", lineHeight: 1.2, marginBottom: 8 }}>
            {coverable ? (
              <>You can do this — <span style={{ color: "var(--accent)" }}>here's what changes</span>.</>
            ) : (
              <>Not without trade-offs — <span style={{ color: "var(--negative)" }}>here's what would have to give</span>.</>
            )}
          </div>
          <div className="muted" style={{ fontSize: 14, lineHeight: 1.55 }}>
            "{scenario.title}"
          </div>
        </div>

        {/* Impact grid */}
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr 1fr", gap: 0, borderTop: "1px solid var(--hairline)" }}>
          <div style={{ padding: 22, borderRight: "1px solid var(--hairline)" }}>
            <div className="eyebrow" style={{ marginBottom: 8 }}>Runway change</div>
            <div className="figure" style={{ fontSize: 32, lineHeight: 1, color: impact.runway >= 0 ? "var(--accent)" : "var(--negative)", letterSpacing: "-0.03em" }}>
              {impact.runway >= 0 ? "+" : ""}{impact.runway}<span style={{ fontSize: 14, color: "var(--ink-mute)", fontWeight: 500, marginLeft: 4 }}>days</span>
            </div>
            <div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>
              {impact.runway >= 0 ? "Adds to your buffer" : "Shortens your buffer"}
            </div>
          </div>
          <div style={{ padding: 22, borderRight: "1px solid var(--hairline)" }}>
            <div className="eyebrow" style={{ marginBottom: 8 }}>Goals affected · {impact.goalsSlip.length}</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
              {impact.goalsSlip.map((g, i) => (
                <div key={i} style={{ fontSize: 13.5, color: "var(--ink-2)" }}>{g}</div>
              ))}
            </div>
          </div>
          <div style={{ padding: 22 }}>
            <div className="eyebrow" style={{ marginBottom: 8 }}>Coverable</div>
            <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
              <span style={{ width: 14, height: 14, borderRadius: 999, background: coverable ? "var(--accent)" : "var(--negative)", boxShadow: "0 0 12px " + (coverable ? "var(--accent)" : "var(--negative)") }}></span>
              <span style={{ fontSize: 18, fontWeight: 500 }}>{coverable ? "Yes" : "Not as planned"}</span>
            </div>
            <div className="muted" style={{ fontSize: 12.5, marginTop: 6 }}>
              {coverable ? "Within your current means" : "You'd need to also adjust spending or income"}
            </div>
          </div>
        </div>
      </div>

      {/* Detailed forecast chart */}
      <div className="bigchart" style={{ marginTop: 14 }}>
        <div className="bigchart-head">
          <div>
            <div className="h3">Cash flow · {forecastRange === "6M" ? "6" : forecastRange === "12M" ? "12" : "24"} months ahead</div>
            <div className="muted" style={{ fontSize: 12.5, marginTop: 4 }}>
              Solid line is your current trajectory. Dashed line is with this scenario applied.
            </div>
          </div>
          <div className="toolbar">
            {["6M", "12M", "24M"].map(r => (
              <button key={r} className={forecastRange === r ? "on" : ""} onClick={() => setForecastRange(r)}>{r}</button>
            ))}
          </div>
        </div>
        <div style={{ padding: "0 22px 6px" }}>
          <ScenarioForecast scenario={scenario} range={forecastRange} />
        </div>
      </div>

      {/* Considerations */}
      <div style={{ marginTop: 14, display: "grid", gridTemplateColumns: "1.2fr 1fr", gap: 14 }}>
        <div className="card">
          <div className="eyebrow" style={{ marginBottom: 14 }}><span className="dot"></span>Worth knowing</div>
          <ol style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 12 }}>
            {[
              "Your emergency fund covers 5 months of expenses. This scenario would draw it down to ~2.5 months at peak.",
              "Health insurance through Adam's job ends 30 days after sabbatical starts — COBRA at ~$1,140/mo or marketplace plan around $740/mo.",
              "Mira's freelance income continues through this period and partially offsets fixed costs.",
              "House Fund contributions pause during sabbatical; September home-buying timeline slips by 9 months in the worst case.",
            ].map((r, i) => (
              <li key={i} style={{ display: "grid", gridTemplateColumns: "22px 1fr", gap: 12, alignItems: "start" }}>
                <span style={{ width: 20, height: 20, borderRadius: 5, background: "var(--surface-2)", border: "1px solid var(--line)", display: "grid", placeItems: "center", fontFamily: "var(--mono)", fontSize: 11, fontWeight: 500, color: "var(--ink-mute)", marginTop: 1 }}>{i + 1}</span>
                <span style={{ fontSize: 13.5, lineHeight: 1.55, color: "var(--ink-2)" }}>{r}</span>
              </li>
            ))}
          </ol>
        </div>
        <div className="card">
          <div className="eyebrow" style={{ marginBottom: 14 }}><span className="dot"></span>What to do</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            <button className="btn primary" style={{ justifyContent: "flex-start", padding: "12px 14px" }} onClick={() => onSave?.(scenario)}>
              <I.Sparkle /> Save this scenario
            </button>
            <button className="btn outline" style={{ justifyContent: "flex-start", padding: "12px 14px" }} onClick={() => window.toast?.("Added to forecast", { kind: "success", sub: "Goals page reflects this constraint" })}>
              <I.Goal /> Add the constraints to your forecast
            </button>
            <button className="btn outline" style={{ justifyContent: "flex-start", padding: "12px 14px" }} onClick={() => window.toast?.("Reminder set for Aug 1", { kind: "success", sub: "You'll get a single morning note" })}>
              <I.Bell /> Set a reminder for August to re-run
            </button>
            <button className="btn ghost" style={{ justifyContent: "flex-start", padding: "12px 14px" }} onClick={() => window.toast?.("Discarded", { kind: "warn" })}>
              <I.X /> Discard
            </button>
          </div>
          <div className="muted" style={{ fontSize: 12, marginTop: 14, lineHeight: 1.5, padding: 12, background: "var(--surface-2)", borderRadius: 8 }}>
            All scenarios are local — nothing happens to your real money until you explicitly apply changes. Run as many as you want.
          </div>
        </div>
      </div>
    </div>
  );
}

function ScenarioForecast({ scenario, range = "12M" }) {
  const W = 1080, H = 240;
  const PAD_L = 48, PAD_R = 14, PAD_T = 16, PAD_B = 30;
  const innerW = W - PAD_L - PAD_R;
  const innerH = H - PAD_T - PAD_B;

  const allMonths = ["Jun","Jul","Aug","Sep","Oct","Nov","Dec","Jan","Feb","Mar","Apr","May","Jun '27","Jul '27","Aug '27","Sep '27","Oct '27","Nov '27","Dec '27","Jan '28","Feb '28","Mar '28","Apr '28","May '28"];
  const monthCount = range === "6M" ? 6 : range === "24M" ? 24 : 12;
  const months = allMonths.slice(0, monthCount);
  // Baseline: gradually rising
  const baseline = months.map((_, i) => 50000 + i * 2400);
  // Scenario impact applied around month 4 (October)
  const scenarioLine = months.map((_, i) => {
    let v = baseline[i];
    if (i >= 4) {
      const ratio = Math.min(1, (i - 4) / 6);
      v = v + (scenario.impact.runway > 0 ? 1 : -1) * Math.abs(scenario.impact.runway) * 30 * ratio;
    }
    return v;
  });
  const max = Math.max(...baseline, ...scenarioLine) * 1.05;
  const min = Math.min(...baseline, ...scenarioLine) * 0.92;
  const yRange = max - min;
  const xFor = (i) => PAD_L + (i / (months.length - 1)) * innerW;
  const yFor = (v) => PAD_T + (1 - (v - min) / yRange) * innerH;

  const linePath = (vals) => vals.map((v, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(v)}`).join(" ");
  const baselinePath = linePath(baseline);
  const scenarioPath = linePath(scenarioLine);

  // Highlight delta region
  const deltaArea = scenarioLine.map((v, i) => `L${xFor(i)},${yFor(v)}`).join(" ");
  const deltaTop = baseline.slice().reverse().map((v, i, arr) => `L${xFor(11 - i)},${yFor(v)}`).join(" ");
  const stressing = scenario.impact.runway < 0;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      <defs>
        <linearGradient id="deltaGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={stressing ? "var(--negative)" : "var(--accent)"} stopOpacity="0.3" />
          <stop offset="100%" stopColor={stressing ? "var(--negative)" : "var(--accent)"} stopOpacity="0.04" />
        </linearGradient>
      </defs>

      {/* gridlines */}
      {[0.25, 0.5, 0.75].map((t, i) => (
        <line key={i} x1={PAD_L} x2={W - PAD_R} y1={PAD_T + innerH * t} y2={PAD_T + innerH * t} stroke="rgba(255,255,255,0.05)" />
      ))}

      {/* delta fill (between baseline and scenario) */}
      <path d={`M${xFor(0)},${yFor(scenarioLine[0])} ${deltaArea} ${deltaTop} Z`} fill="url(#deltaGrad)" />

      {/* baseline (current trajectory) */}
      <path d={baselinePath} stroke="var(--ink)" strokeWidth="1.8" fill="none" />

      {/* scenario (with this applied) */}
      <path d={scenarioPath} stroke={stressing ? "var(--negative)" : "var(--accent)"} strokeWidth="2" fill="none" strokeDasharray="6 4" />
      {scenarioLine.map((v, i) => i === 4 && (
        <g key={i}>
          <line x1={xFor(i)} x2={xFor(i)} y1={PAD_T} y2={H - PAD_B} stroke={stressing ? "var(--negative)" : "var(--accent)"} strokeWidth="0.8" strokeDasharray="2 3" opacity="0.5" />
          <text x={xFor(i) + 6} y={PAD_T + 14} style={{ fontFamily: "var(--mono)", fontSize: 11, fill: stressing ? "var(--negative)" : "var(--accent)", fontWeight: 500 }}>scenario starts</text>
        </g>
      ))}

      {/* dots at end */}
      <circle cx={xFor(months.length - 1)} cy={yFor(baseline[months.length - 1])} r="4" fill="var(--ink)" stroke="var(--bg)" strokeWidth="2" />
      <circle cx={xFor(months.length - 1)} cy={yFor(scenarioLine[months.length - 1])} r="4" fill={stressing ? "var(--negative)" : "var(--accent)"} stroke="var(--bg)" strokeWidth="2" />

      {/* month labels */}
      {months.map((m, i) => (i % Math.ceil(months.length / 12) === 0) && (
        <text key={i} x={xFor(i)} y={H - 10} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>{m}</text>
      ))}
      {/* y ticks */}
      {[0.25, 0.5, 0.75].map((t, i) => {
        const v = min + yRange * (1 - t);
        return (
          <text key={i} x={PAD_L - 6} y={PAD_T + innerH * t + 3} textAnchor="end" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>${Math.round(v / 1000)}k</text>
        );
      })}

      {/* legend */}
      <g transform={`translate(${PAD_L} 4)`}>
        <rect x="0" y="0" width="14" height="3" fill="var(--ink)" />
        <text x="20" y="4" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>current path</text>
        <rect x="120" y="0" width="14" height="3" fill={stressing ? "var(--negative)" : "var(--accent)"} />
        <text x="140" y="4" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>with scenario</text>
      </g>
    </svg>
  );
}

window.Scenarios = Scenarios;
