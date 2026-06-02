/* Today — the morning briefing.
   Big bold net-worth hero (Copilot-style), gradient area chart,
   agent activity, upcoming, category bar.
*/

function NetWorthChart({ history }) {
  const W = 1200, H = 280;
  const PAD_L = 14, PAD_R = 14, PAD_T = 30, PAD_B = 36;
  const innerW = W - PAD_L - PAD_R;
  const innerH = H - PAD_T - PAD_B;

  const vals = history.map(d => d.v);
  const min = Math.min(...vals) * 0.92;
  const max = Math.max(...vals) * 1.05;
  const range = max - min || 1;
  const xFor = (i) => PAD_L + (i / (history.length - 1)) * innerW;
  const yFor = (v) => PAD_T + (1 - (v - min) / range) * innerH;

  const linePath = history.map((d, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(d.v)}`).join(" ");
  const areaPath = `${linePath} L${xFor(history.length - 1)},${H - PAD_B} L${PAD_L},${H - PAD_B} Z`;

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      <defs>
        <linearGradient id="nwGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor="var(--accent)" stopOpacity="0.34" />
          <stop offset="60%" stopColor="var(--accent)" stopOpacity="0.06" />
          <stop offset="100%" stopColor="var(--accent)" stopOpacity="0" />
        </linearGradient>
        <filter id="nwGlow" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation="4" />
        </filter>
      </defs>

      {/* grid lines */}
      {[0.25, 0.5, 0.75].map((t, i) => (
        <line key={i} x1={PAD_L} x2={W - PAD_R} y1={PAD_T + innerH * t} y2={PAD_T + innerH * t}
              stroke="rgba(255,255,255,0.04)" strokeWidth="1" />
      ))}

      {/* area */}
      <path d={areaPath} fill="url(#nwGrad)" />

      {/* glow behind line */}
      <path d={linePath} fill="none" stroke="var(--accent)" strokeWidth="3.5" strokeLinejoin="round" strokeLinecap="round" filter="url(#nwGlow)" opacity="0.5" />
      {/* line */}
      <path d={linePath} fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinejoin="round" strokeLinecap="round" />

      {/* dots */}
      {history.map((d, i) => {
        const isLast = i === history.length - 1;
        return (
          <g key={i}>
            <circle cx={xFor(i)} cy={yFor(d.v)} r={isLast ? 6 : 3.2}
                    fill={isLast ? "var(--accent)" : "var(--accent)"} stroke="var(--bg)" strokeWidth="2" />
            {isLast && <circle cx={xFor(i)} cy={yFor(d.v)} r="14" fill="var(--accent)" opacity="0.25" />}
          </g>
        );
      })}

      {/* month labels */}
      {history.map((d, i) => (
        <text key={i} x={xFor(i)} y={H - 12} textAnchor="middle"
              style={{ fontFamily: "var(--mono)", fontSize: 11.5, fill: "var(--ink-faint)" }}>
          {d.m}
        </text>
      ))}

      {/* value label on last point */}
      <g>
        <text x={xFor(history.length - 1)} y={yFor(history[history.length - 1].v) - 20} textAnchor="end"
              style={{ fontFamily: "var(--sans)", fontSize: 14, fontWeight: 600, fill: "var(--accent)" }}>
          ${(history[history.length - 1].v / 1000).toFixed(1)}k
        </text>
      </g>
    </svg>
  );
}

function CashflowMini({ cashflow }) {
  const W = 380, H = 64;
  const vals = cashflow.map(d => d.running);
  const min = Math.min(...vals) * 0.98;
  const max = Math.max(...vals) * 1.02;
  const range = max - min || 1;
  const xFor = (i) => (i / (cashflow.length - 1)) * W;
  const yFor = (v) => H - ((v - min) / range) * (H - 8) - 4;

  const splitIndex = cashflow.findIndex(d => d.forecast);
  const actualPath = cashflow.slice(0, splitIndex).map((d, i) => `${i === 0 ? "M" : "L"}${xFor(i)},${yFor(d.running)}`).join(" ");
  const fcPath = cashflow.slice(splitIndex - 1).map((d, i) => `${i === 0 ? "M" : "L"}${xFor(i + splitIndex - 1)},${yFor(d.running)}`).join(" ");
  const todayX = xFor(splitIndex - 1);

  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" height={H} preserveAspectRatio="none">
      <path d={actualPath} fill="none" stroke="var(--ink)" strokeWidth="1.5" />
      <path d={fcPath} fill="none" stroke="var(--accent)" strokeWidth="1.5" strokeDasharray="2 3" />
      <line x1={todayX} x2={todayX} y1="2" y2={H - 2} stroke="var(--accent)" strokeWidth="0.8" opacity="0.5" />
    </svg>
  );
}

function Today({ setRoute }) {
  const { totals, agentActivity, cashflow, recurring, today, netWorthHistory, categories } = FS;
  const upcoming = recurring
    .filter(r => ["May 22", "May 24", "May 25", "May 28"].includes(r.next))
    .sort((a, b) => a.next.localeCompare(b.next));
  const [nwRange, setNwRange] = React.useState("6M");
  const [sweepDismissed, setSweepDismissed] = React.useState(false);
  const [anomalyDismissed, setAnomalyDismissed] = React.useState(false);
  const [activityShown, setActivityShown] = React.useState(5);

  const visibleHistory = (() => {
    const m = { "1M": 1, "3M": 3, "6M": 6, "1Y": 6, "All": 6 }[nwRange] || 6;
    return netWorthHistory.slice(-m);
  })();

  // Animated number
  const target = totals.netWorth;
  const [n, setN] = React.useState(target * 0.85);
  React.useEffect(() => {
    let f;
    const start = performance.now();
    const from = target * 0.85;
    const tick = (t) => {
      const e = Math.min(1, (t - start) / 700);
      const eased = 1 - Math.pow(1 - e, 3);
      setN(from + (target - from) * eased);
      if (e < 1) f = requestAnimationFrame(tick);
    };
    f = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(f);
  }, [target]);

  const big = Math.round(n).toLocaleString("en-US");

  const totalSpent = categories.reduce((s, c) => s + c.thisMonth, 0);
  const lastTotal = categories.reduce((s, c) => s + c.lastMonth, 0);
  const monthDelta = totalSpent - lastTotal;

  return (
    <div className="screen">
      {/* Date + status row */}
      <div className="day-hdr">
        <div className="eyebrow">
          <span className="dot"></span>{today.dow} · {today.m} {today.d}, {today.y}
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <span className="chip"><I.Lock width="11" height="11" /> Local-only</span>
          <span className="chip accent"><span className="dot"></span>Agent · ran 10m ago</span>
        </div>
      </div>

      {/* Hero number */}
      <div className="hero-num">
        <div className="eyebrow" style={{ color: "var(--ink-mute)" }}>Net worth</div>
        <div className="h-display">
          <span className="figure">$<span className="figure">{big}</span></span>
        </div>
        <div className="hero-meta">
          <span className="npill pos">↑ $3,220</span>
          <span className="muted">in the last 30 days</span>
          <span className="muted">·</span>
          <span>You’re tracking <span className="strong">11% below</span> April spending.</span>
        </div>
      </div>

      {/* Net worth chart */}
      <div className="bigchart">
        <div className="bigchart-head">
          <div>
            <div className="h3">Net worth · last {nwRange === "1M" ? "month" : nwRange === "3M" ? "3 months" : nwRange === "6M" ? "6 months" : nwRange === "1Y" ? "year" : "all time"}</div>
            <div className="muted" style={{ fontSize: 13, marginTop: 4 }}>Assets minus liabilities, marked monthly.</div>
          </div>
          <div className="toolbar">
            {["1M", "3M", "6M", "1Y", "All"].map(r => (
              <button key={r} className={nwRange === r ? "on" : ""} onClick={() => setNwRange(r)}>{r}</button>
            ))}
          </div>
        </div>
        <div style={{ padding: "0 14px" }}>
          <NetWorthChart history={visibleHistory} />
        </div>
      </div>

      {/* Stat row */}
      <div className="stat-row">
        <div className="stat">
          <div className="label">Liquid</div>
          <div className="value">$49,513<span className="small">.60</span></div>
          <div className="sub"><span className="npill pos">+$1,980</span> across 4 accounts</div>
        </div>
        <div className="stat">
          <div className="label">Invested</div>
          <div className="value">$86,420</div>
          <div className="sub"><span className="npill pos">+$1,200</span> retirement</div>
        </div>
        <div className="stat">
          <div className="label">Credit</div>
          <div className="value">$2,418</div>
          <div className="sub"><span className="chip negative" style={{ padding: "1px 7px" }}><span className="dot"></span>Statement closes in 3d</span></div>
        </div>
        <div className="stat accent">
          <div className="label">Runway · at current burn</div>
          <div className="value">134<span className="small">days</span></div>
          <div className="sub">$5,980/mo · ends Oct 1, 2026</div>
        </div>
      </div>

      {/* Morning briefing prose + Smart sweep */}
      <div className="section" style={{ display: "grid", gridTemplateColumns: "1.4fr 1fr", gap: 18, marginTop: 22 }}>
        <div className="card" style={{ padding: 26 }}>
          <div className="eyebrow" style={{ marginBottom: 14 }}><span className="dot"></span>Morning briefing · 60 seconds</div>
          <p style={{ fontSize: 16, lineHeight: 1.6, margin: 0, color: "var(--ink-2)", textWrap: "pretty" }}>
            Tuesday morning. <span className="strong" style={{ color: "var(--ink)" }}>Net worth up $3,220 over the last 30 days</span>, mostly from skipping the Lisbon trip and a small bump in retirement. <span className="strong" style={{ color: "var(--ink)" }}>This week brings rent already paid, an Amex statement closing Friday, and a free trial expiring Saturday</span> — the agent has scheduled actions for both. One <span style={{ color: "var(--negative)" }}>anomaly worth a glance</span>: your PG&E bill is 2.1× the usual. Otherwise: calm.
          </p>
          <div style={{ marginTop: 18, display: "flex", gap: 8 }}>
            <button className="btn outline sm" onClick={() => window.toast?.("Audio briefing started", { sub: "≈60 seconds · narrated by Mira's voice", kind: "accent" })}>Hear it instead</button>
            <button className="btn outline sm" onClick={() => setRoute("insights")}><I.Sparkle /> Read full insights</button>
            <button className="btn ghost sm" onClick={() => window.openCmd?.()}>Ask follow-up <span className="kbd" style={{ marginLeft: 4 }}>⌘K</span></button>
          </div>
        </div>

        {!sweepDismissed && (
        <div className="card" style={{ padding: 0, overflow: "hidden", background: "linear-gradient(180deg, var(--accent-2) 0%, var(--surface) 70%)", border: "1px solid var(--accent-3)" }}>
          <div style={{ padding: 22 }}>
            <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot"></span>Smart sweep · suggested</div>
            <p style={{ fontSize: 14, lineHeight: 1.55, margin: 0, color: "var(--ink-2)" }}>
              Joint Checking is <span className="strong">$2,400 above</span> your usual end-of-month floor. Sweep some toward the House Fund?
            </p>
            <div style={{ marginTop: 16, padding: 14, background: "var(--surface)", borderRadius: 10, border: "1px solid var(--line)" }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                <div>
                  <div className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.06em" }}>Move</div>
                  <div className="figure" style={{ fontSize: 28, marginTop: 4 }}>$1,500</div>
                  <div className="muted" style={{ fontSize: 12.5, marginTop: 2 }}>Joint Checking → House Fund</div>
                </div>
                <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                  <button className="btn primary sm" onClick={() => {
                    setSweepDismissed(true);
                    window.toast?.("Moved $1,500 to House Fund", { kind: "success", sub: "Joint Checking → Wealthfront", action: { label: "Undo", onClick: () => { setSweepDismissed(false); window.toast?.("Sweep undone"); } } });
                  }}>Do it</button>
                  <button className="btn ghost sm" onClick={() => window.toast?.("Opened sweep editor", { sub: "Adjust the amount or destination" })}>Adjust</button>
                  <button className="btn ghost sm" onClick={() => { setSweepDismissed(true); window.toast?.("Suggestion dismissed"); }}>Dismiss</button>
                </div>
              </div>
            </div>
          </div>
        </div>
        )}
      </div>

      {/* Anomaly card */}
      {!anomalyDismissed && (
      <div className="card" style={{ marginTop: 18, padding: 22, border: "1px solid var(--negative)", background: "linear-gradient(90deg, var(--negative-2) 0%, var(--surface) 60%)" }}>
        <div style={{ display: "grid", gridTemplateColumns: "auto 1fr auto", gap: 18, alignItems: "center" }}>
          <div style={{ width: 40, height: 40, borderRadius: 10, background: "var(--negative-2)", color: "var(--negative)", display: "grid", placeItems: "center" }}>
            <I.Bell />
          </div>
          <div>
            <div className="eyebrow" style={{ marginBottom: 4, color: "var(--negative)" }}><span className="dot" style={{ background: "var(--negative)", boxShadow: "0 0 8px var(--negative)" }}></span>Anomaly · PG&E</div>
            <div style={{ fontSize: 14.5, color: "var(--ink)", lineHeight: 1.5 }}>
              Your <span className="strong">$220</span> electric bill on May 10 is <span className="strong" style={{ color: "var(--negative)" }}>2.1× your 12-month average ($105)</span>. Last spike like this was January's cold snap.
            </div>
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button className="btn sm" onClick={() => setRoute("transactions")}>Investigate</button>
            <button className="btn ghost sm" onClick={() => { setAnomalyDismissed(true); window.toast?.("Anomaly dismissed", { sub: "Marked as expected · won't reappear" }); }}>Dismiss</button>
          </div>
        </div>
      </div>
      )}
      <div className="card" style={{ marginTop: 22 }}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 14 }}>
          <div>
            <div className="eyebrow" style={{ marginBottom: 6 }}>Spent this month</div>
            <div style={{ display: "flex", alignItems: "baseline", gap: 14 }}>
              <div className="figure" style={{ fontSize: 32 }}>${totalSpent.toLocaleString()}</div>
              <span className={`npill ${monthDelta < 0 ? "pos" : "neg"}`}>
                {monthDelta < 0 ? "↓" : "↑"} ${Math.abs(monthDelta).toLocaleString()} vs Apr
              </span>
            </div>
          </div>
          <button className="btn outline sm" onClick={() => setRoute("categories")}>Open categories <I.ArrowR /></button>
        </div>
        <div className="stream" style={{ height: 10 }}>
          {categories.filter(c => c.thisMonth > 0).map(c => (
            <span key={c.id} title={`${c.label} · ${FS.fmt(c.thisMonth)}`} style={{ width: `${(c.thisMonth / totalSpent) * 100}%`, background: c.color }} />
          ))}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(5, 1fr)", gap: 14, marginTop: 18 }}>
          {categories.filter(c => c.thisMonth > 0).slice(0, 10).map(c => (
            <div key={c.id}>
              <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                <span className="cswatch" style={{ background: c.color }} />
                <span style={{ fontSize: 13, color: "var(--ink-mute)" }}>{c.label}</span>
              </div>
              <div className="figure" style={{ fontSize: 19, marginTop: 6, lineHeight: 1 }}>${c.thisMonth.toLocaleString()}</div>
              <div style={{ marginTop: 4, fontSize: 11.5, color: c.thisMonth < c.lastMonth ? "var(--positive)" : "var(--negative)", fontFamily: "var(--mono)" }}>
                {c.thisMonth < c.lastMonth ? "↓" : "↑"} ${Math.abs(c.thisMonth - c.lastMonth).toLocaleString()}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Agent + Upcoming */}
      <div className="section" style={{ display: "grid", gridTemplateColumns: "1.2fr 1fr", gap: 20 }}>
        <div className="card">
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
            <div>
              <div className="eyebrow" style={{ marginBottom: 4 }}><span className="dot"></span>Agent · while you were away</div>
              <div className="h3" style={{ marginTop: 2 }}>5 things happened</div>
            </div>
            <button className="btn ghost sm" onClick={() => setRoute("insights")}>Review all <I.ArrowR /></button>
          </div>
          <div className="act">
            {agentActivity.map((a, i) => {
              const Ico = a.kind === "ok" ? I.Check : a.kind === "warn" ? I.Bell : I.Sparkle;
              return (
                <div className="act-item" key={i}>
                  <span className={`ic ${a.kind}`}><Ico /></span>
                  <div className="txt">
                    {a.title}
                    <div className="sub">{a.sub}</div>
                  </div>
                  <span className="when">{a.when}</span>
                </div>
              );
            })}
          </div>
        </div>

        <div className="card">
          <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 12 }}>
            <div>
              <div className="eyebrow" style={{ marginBottom: 4 }}><span className="dot"></span>Looking ahead</div>
              <div className="h3" style={{ marginTop: 2 }}>Next 10 days</div>
            </div>
            <button className="btn ghost sm" onClick={() => setRoute("recurring")}>All recurring <I.ArrowR /></button>
          </div>
          <table className="tbl" style={{ margin: "0 -8px" }}>
            <tbody>
              {upcoming.map(u => (
                <tr key={u.id}>
                  <td style={{ width: 64, color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 12.5 }}>{u.next}</td>
                  <td>
                    <div style={{ display: "flex", flexDirection: "column" }}>
                      <span style={{ fontSize: 13.5 }}>{u.name}</span>
                      {u.note && <span style={{ fontSize: 11.5, color: "var(--negative)", marginTop: 2 }}>{u.note}</span>}
                    </div>
                  </td>
                  <td className="right num tabular" style={{ color: u.amount < 0 ? "var(--ink)" : "var(--positive)" }}>{FS.fmt(u.amount, { decimals: 2 })}</td>
                </tr>
              ))}
              <tr>
                <td style={{ color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 12.5 }}>May 31</td>
                <td className="muted">Projected closing balance</td>
                <td className="right figure" style={{ fontSize: 14 }}>$42,612</td>
              </tr>
            </tbody>
          </table>
          <div style={{ marginTop: 18, paddingTop: 14, borderTop: "1px solid var(--hairline)" }}>
            <div className="eyebrow" style={{ marginBottom: 8 }}>Month so far</div>
            <CashflowMini cashflow={cashflow} />
          </div>
        </div>
      </div>
    </div>
  );
}

window.Today = Today;
