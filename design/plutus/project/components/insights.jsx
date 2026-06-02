/* Insights — the agent's findings, visually rendered as analysis traces, not text. */

const KIND_META = {
  pattern:      { label: "Pattern",        color: "#60A5FA", ico: "↗", verb: "noticed a pattern" },
  anomaly:      { label: "Anomaly",        color: "#FB7185", ico: "!", verb: "flagged an anomaly" },
  subscription: { label: "Subscription",   color: "#F472B6", ico: "□", verb: "reviewed a subscription" },
  goal:         { label: "Goal coaching",  color: "#FCA5A5", ico: "○", verb: "modelled your goals" },
  forecast:     { label: "Forecast",       color: "#2DD4BF", ico: "→", verb: "projected a milestone" },
  comparison:   { label: "Vs your budget", color: "#FACC15", ico: "≠", verb: "compared to your budget" },
  tags:         { label: "Your tags",      color: "#FDE68A", ico: "#", verb: "audited your tags" },
};

function Insights() {
  const [allInsights, setAllInsights] = React.useState(FS.insights);
  const [memory, setMemory] = React.useState(FS.agentMemory);
  const [filter, setFilter] = React.useState("all");
  const [expanded, setExpanded] = React.useState(new Set([FS.insights[0].id, FS.insights[1].id]));
  const [scanIdx, setScanIdx] = React.useState(0);
  const [scanning, setScanning] = React.useState(false);

  // Cycling "currently watching" ticker
  const watchItems = [
    "Joint Checking balance · stable",
    "Recurring price changes · 1 flagged",
    "Saturday spending pattern · within range",
    "Goal pace drift · all 5 active goals on track",
    "Anomaly detection · 1 active (PG&E)",
    "Tag completeness · 9 missing receipts",
  ];
  React.useEffect(() => {
    const t = setInterval(() => setScanIdx(i => (i + 1) % watchItems.length), scanning ? 350 : 2400);
    return () => clearInterval(t);
  }, [scanning]);

  const counts = allInsights.reduce((m, i) => ({ ...m, [i.kind]: (m[i.kind] || 0) + 1 }), {});
  const filtered = filter === "all" ? allInsights : allInsights.filter(i => i.kind === filter);

  const toggle = (id) => {
    setExpanded(s => {
      const next = new Set(s);
      if (next.has(id)) next.delete(id); else next.add(id);
      return next;
    });
  };

  const dismiss = (id) => {
    const item = allInsights.find(x => x.id === id);
    setAllInsights(list => list.filter(x => x.id !== id));
    window.toast?.("Insight dismissed", {
      sub: item?.headline.slice(0, 56) + (item?.headline.length > 56 ? "…" : ""),
      action: { label: "Undo", onClick: () => setAllInsights(list => [item, ...list]) },
    });
  };
  const actOn = (id, label) => {
    const item = allInsights.find(x => x.id === id);
    setAllInsights(list => list.filter(x => x.id !== id));
    window.toast?.(`\u201c${label}\u201d applied`, {
      kind: "success",
      sub: item?.headline.slice(0, 56) + (item?.headline.length > 56 ? "…" : ""),
    });
  };
  const reRun = () => {
    setScanning(true);
    window.toast?.("Re-running scan", { sub: "1,247 transactions · local model", kind: "accent" });
    setTimeout(() => {
      setScanning(false);
      window.toast?.("Scan complete · nothing new", { kind: "success", sub: `${allInsights.length} insights, same as before` });
    }, 2400);
  };
  const markAll = () => {
    if (!allInsights.length) return window.toast?.("Nothing to mark");
    window.toast?.(`${allInsights.length} insights marked reviewed`, { kind: "success" });
    setAllInsights([]);
  };
  const forgetMem = (id) => {
    const m = memory.find(x => x.id === id);
    setMemory(list => list.filter(x => x.id !== id));
    window.toast?.("Forgotten", { kind: "warn", sub: m?.learned, action: { label: "Undo", onClick: () => setMemory(list => [m, ...list]) } });
  };

  return (
    <div className="screen">
      {/* Operator panel — make the agent presence felt */}
      <div className="card" style={{ padding: 0, overflow: "hidden", border: "1px solid var(--accent-3)", background: "linear-gradient(135deg, var(--accent-2) 0%, var(--surface) 100%)" }}>
        <div style={{ padding: "24px 28px 22px", display: "grid", gridTemplateColumns: "1fr auto", gap: 28, alignItems: "center" }}>
          <div>
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 16 }}>
              <div style={{ position: "relative", width: 12, height: 12 }}>
                <span style={{ position: "absolute", inset: 0, borderRadius: 999, background: "var(--accent)", animation: "pulse 1.6s infinite" }}></span>
                <span style={{ position: "absolute", inset: 3, borderRadius: 999, background: "var(--accent)", boxShadow: "0 0 12px var(--accent)" }}></span>
              </div>
              <span className="eyebrow" style={{ color: "var(--accent)" }}>Agent · running locally</span>
              <span style={{ width: 4, height: 4, borderRadius: 999, background: "var(--ink-faint)" }}></span>
              <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>llama-3.3 (8B) · 12 min ago · 1,247 tx analyzed</span>
            </div>
            <h1 className="h1" style={{ fontSize: 36, lineHeight: 1.05, letterSpacing: "-0.03em", marginBottom: 10 }}>
              <span style={{ color: "var(--accent)" }}>{allInsights.length}</span> things the agent noticed.
            </h1>
            <p className="muted" style={{ fontSize: 14.5, lineHeight: 1.55, maxWidth: "64ch", margin: 0 }}>
              Every card below is a finding derived from your transactions, balances, budgets, and the tags you've added — with the full reasoning trace shown. Read, dismiss, or act. The agent learns from what you skip.
            </p>
          </div>

          {/* Watching ticker */}
          <div style={{ minWidth: 280, padding: 18, background: "rgba(0,0,0,0.25)", border: "1px solid var(--line)", borderRadius: 12 }}>
            <div className="eyebrow" style={{ marginBottom: 12, color: "var(--ink-mute)" }}>
              <span className="dot" style={{ background: "var(--accent)", boxShadow: "0 0 6px var(--accent)" }}></span>Currently watching
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: 6, fontFamily: "var(--mono)", fontSize: 12 }}>
              {watchItems.map((w, i) => {
                const isCur = i === scanIdx;
                const isPast = (scanIdx - i + watchItems.length) % watchItems.length <= 2 && i !== scanIdx;
                return (
                  <div key={i} style={{
                    display: "flex", alignItems: "center", gap: 8,
                    color: isCur ? "var(--accent)" : isPast ? "var(--ink-mute)" : "var(--ink-faint)",
                    opacity: isCur ? 1 : 0.55,
                    transition: "color 0.4s, opacity 0.4s",
                  }}>
                    <span style={{ width: 6, opacity: isCur ? 1 : 0.3 }}>{isCur ? "▸" : " "}</span>
                    <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{w}</span>
                  </div>
                );
              })}
            </div>
          </div>
        </div>

        {/* Scope strip */}
        <div style={{ padding: "12px 28px", borderTop: "1px solid var(--line)", display: "flex", justifyContent: "space-between", alignItems: "center", background: "rgba(0,0,0,0.2)" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <I.Lock width="13" height="13" style={{ color: "var(--ink-mute)" }} />
            <span className="muted" style={{ fontSize: 12 }}>
              Reads <span className="strong">your transactions, balances, budgets, goals, tags</span> — nothing else. No cloud, no third-party data.
            </span>
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button className="btn ghost sm" onClick={markAll}><I.Check /> Mark all reviewed</button>
            <button className="btn sm" onClick={reRun} disabled={scanning} style={{ opacity: scanning ? 0.6 : 1 }}>
              <I.Sparkle /> {scanning ? "Scanning…" : "Re-run scan"}
            </button>
          </div>
        </div>
      </div>

      {/* Stat strip — kind counts */}
      <div className="stat-row" style={{ marginTop: 22 }}>
        <Stat label="Patterns noticed" value={(counts.pattern || 0).toString()} sub={<span className="muted" style={{ fontSize: 12.5 }}>across 14 months</span>} />
        <Stat label="Anomalies" value={(counts.anomaly || 0).toString()} sub={<span className="npill neg">needs glance</span>} />
        <Stat label="Forecasts" value={(counts.forecast || 0).toString()} sub={<span className="muted" style={{ fontSize: 12.5 }}>linear projections</span>} />
        <Stat label="Budget alerts" value={((counts.comparison || 0) + (counts.goal || 0)).toString()} sub={<span className="muted" style={{ fontSize: 12.5 }}>vs your settings</span>} accent />
      </div>

      {/* Filter chips */}
      <div style={{ marginTop: 28, display: "flex", gap: 8, flexWrap: "wrap", alignItems: "center" }}>
        <FilterChip on={filter === "all"} onClick={() => setFilter("all")}>All · {allInsights.length}</FilterChip>
        {Object.entries(KIND_META).map(([k, m]) => (
          counts[k] && (
            <FilterChip key={k} on={filter === k} onClick={() => setFilter(k)} color={m.color}>
              <span style={{ width: 6, height: 6, borderRadius: 999, background: m.color, boxShadow: `0 0 6px ${m.color}` }}></span>
              {m.label} · {counts[k]}
            </FilterChip>
          )
        ))}
        <span style={{ marginLeft: "auto", color: "var(--ink-faint)", fontSize: 12, fontFamily: "var(--mono)" }}>sorted by severity · newest first</span>
      </div>

      {/* Feed */}
      <div style={{ marginTop: 22, display: "flex", flexDirection: "column", gap: 14 }}>
        {filtered.length === 0 && (
          <div className="empty-dash" style={{ marginTop: 8 }}>
            <div className="h1" style={{ fontSize: 22 }}>Nothing to review.</div>
            <div className="muted" style={{ fontSize: 14, marginTop: 8 }}>The agent will surface new findings as they appear.</div>
          </div>
        )}
        {filtered.map(i => <InsightCard key={i.id} insight={i} expanded={expanded.has(i.id)} onToggle={() => toggle(i.id)} onDismiss={() => dismiss(i.id)} onAct={(label) => actOn(i.id, label)} />)}
      </div>

      {/* Agent memory */}
      <div className="section">
        <div className="section-hdr">
          <div>
            <div className="eyebrow"><span className="dot"></span>Agent memory</div>
            <h2 className="h1" style={{ fontSize: 22 }}>What it's learned from you.</h2>
          </div>
          <button className="btn ghost sm" onClick={() => window.toast?.("Editing memory", { sub: "Click any ‘Forget’ to remove an item", duration: 4000 })}>Edit / forget</button>
        </div>
        <div className="card flush">
          {memory.length === 0 && (
            <div style={{ padding: 32, textAlign: "center", color: "var(--ink-faint)", fontSize: 13 }}>
              No memory items. The agent will re-learn as you correct it.
            </div>
          )}
          {memory.map((m, i) => (
            <div key={m.id} style={{ display: "grid", gridTemplateColumns: "1fr auto auto auto", gap: 18, padding: "16px 22px", borderBottom: i === memory.length - 1 ? "0" : "1px solid var(--hairline)", alignItems: "center" }}>
              <div>
                <div style={{ fontSize: 14, color: "var(--ink)" }}>{m.learned}</div>
                <div className="muted" style={{ fontSize: 12, marginTop: 3, fontFamily: "var(--mono)" }}>{m.source} · since {m.since}</div>
              </div>
              <span className={`chip ${m.weight === "high" ? "accent" : ""}`} style={{ padding: "3px 9px" }}>{m.weight} confidence</span>
              <button className="btn ghost sm" onClick={() => forgetMem(m.id)}>Forget</button>
              <button className="btn ghost sm" onClick={() => window.toast?.("Memory details", { sub: m.learned, duration: 3000 })}><I.More /></button>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function FilterChip({ on, onClick, color, children }) {
  return (
    <button onClick={onClick}
      style={{
        display: "inline-flex", alignItems: "center", gap: 7,
        padding: "7px 14px",
        borderRadius: 999,
        fontSize: 13,
        fontWeight: 500,
        background: on ? "var(--surface)" : "transparent",
        color: on ? "var(--ink)" : "var(--ink-mute)",
        border: "1px solid " + (on ? "var(--line-2)" : "var(--line)"),
        cursor: "pointer",
      }}>
      {children}
    </button>
  );
}

function InsightCard({ insight, expanded, onToggle, onDismiss, onAct }) {
  const meta = KIND_META[insight.kind] || { label: insight.kind, color: "var(--ink-mute)" };
  const sevColor = insight.severity >= 3 ? "var(--negative)" :
                   insight.severity >= 2 ? "var(--warning)"  :
                   insight.severity >= 1 ? meta.color         :
                                            "var(--ink-faint)";
  const isAnomaly = insight.kind === "anomaly";
  const isHigh = insight.severity >= 3;

  // Confidence (mock) — derived from reasoning length
  const conf = Math.min(99, 70 + insight.reasoning.length * 5);

  return (
    <div style={{
      position: "relative",
      background: "var(--surface)",
      border: "1px solid " + (isHigh ? "var(--negative)" : "var(--line)"),
      borderRadius: 14,
      overflow: "hidden",
    }}>
      {/* Accent edge */}
      <div style={{
        position: "absolute", left: 0, top: 0, bottom: 0,
        width: 3,
        background: meta.color,
        boxShadow: `0 0 12px ${meta.color}66`,
      }}></div>

      {/* Header */}
      <div style={{ padding: "22px 26px 18px 30px", cursor: "pointer" }} onClick={onToggle}>
        <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 14 }}>
          {/* Animated glyph */}
          <div style={{ position: "relative" }}>
            {isAnomaly && (
              <span style={{
                position: "absolute", inset: -4, borderRadius: 999,
                background: meta.color, opacity: 0.4,
                animation: "pulse 1.8s infinite",
              }}></span>
            )}
            <span style={{
              position: "relative",
              width: 26, height: 26, borderRadius: 7,
              background: `color-mix(in oklab, ${meta.color} 22%, transparent)`,
              border: `1px solid color-mix(in oklab, ${meta.color} 35%, transparent)`,
              color: meta.color,
              display: "grid", placeItems: "center",
              fontFamily: "var(--mono)", fontSize: 13, fontWeight: 600,
            }}>{meta.ico}</span>
          </div>

          <span className="eyebrow" style={{ color: meta.color }}>Agent {meta.verb}</span>
          <span style={{ width: 4, height: 4, borderRadius: 999, background: "var(--ink-faint)" }}></span>
          <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>{insight.age}</span>

          <span style={{ flex: 1 }}></span>

          {/* Confidence */}
          <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
            <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>conf {conf}%</span>
            <div style={{ width: 40, height: 4, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
              <div style={{ width: conf + "%", height: "100%", background: meta.color, borderRadius: 999 }}></div>
            </div>
          </div>

          <button className="btn ghost sm" style={{ padding: "3px 6px", marginLeft: 6 }}>{expanded ? <I.Up /> : <I.Down />}</button>
        </div>

        <div style={{ fontSize: 22, fontWeight: 500, letterSpacing: "-0.022em", lineHeight: 1.25, color: "var(--ink)", textWrap: "pretty" }}>
          {insight.headline}
        </div>
        <div className="muted" style={{ fontSize: 14, marginTop: 8, lineHeight: 1.55, textWrap: "pretty" }}>
          {insight.summary}
        </div>

        {/* Mini visualization when collapsed (only some kinds) */}
        {!expanded && insight.data && insight.data.kind === "bars" && (
          <div style={{ marginTop: 16, display: "flex", alignItems: "flex-end", gap: 4, height: 32 }}>
            {insight.data.values.map((v, i) => {
              const max = Math.max(...insight.data.values);
              return (
                <div key={i} style={{
                  flex: 1,
                  height: (v / max) * 100 + "%",
                  background: i % 2 === 1 ? meta.color : `color-mix(in oklab, ${meta.color} 30%, transparent)`,
                  borderRadius: 2,
                  minHeight: 4,
                }}></div>
              );
            })}
          </div>
        )}
      </div>

      {/* Expanded — reasoning trace */}
      {expanded && (
        <div style={{ borderTop: "1px solid var(--hairline)", background: "rgba(0,0,0,0.18)" }}>
          <div style={{ padding: "20px 26px 20px 30px" }}>
            <div style={{ display: "flex", alignItems: "center", gap: 10, marginBottom: 18 }}>
              <span style={{
                fontFamily: "var(--mono)", fontSize: 10.5,
                padding: "3px 8px",
                background: `color-mix(in oklab, ${meta.color} 18%, transparent)`,
                color: meta.color,
                borderRadius: 4,
                letterSpacing: "0.06em",
                textTransform: "uppercase",
                fontWeight: 600,
              }}>// reasoning trace</span>
              <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>{insight.reasoning.length} steps</span>
            </div>

            {/* Connected reasoning trail */}
            <div style={{ position: "relative", paddingLeft: 0 }}>
              <ol style={{ margin: 0, padding: 0, listStyle: "none", display: "flex", flexDirection: "column", gap: 14 }}>
                {insight.reasoning.map((r, i) => {
                  const isLast = i === insight.reasoning.length - 1;
                  return (
                    <li key={i} style={{ display: "grid", gridTemplateColumns: "26px 1fr", gap: 14, alignItems: "start", position: "relative" }}>
                      {/* Numbered node + connecting line */}
                      <div style={{ position: "relative", display: "flex", justifyContent: "center", height: "100%" }}>
                        <span style={{
                          width: 22, height: 22, borderRadius: 999,
                          background: "var(--surface)",
                          border: `1.5px solid ${meta.color}`,
                          display: "grid", placeItems: "center",
                          fontFamily: "var(--mono)", fontSize: 10.5, fontWeight: 600,
                          color: meta.color,
                          flexShrink: 0,
                          marginTop: 1,
                          position: "relative",
                          zIndex: 1,
                        }}>{i + 1}</span>
                        {!isLast && (
                          <div style={{
                            position: "absolute",
                            left: "50%", top: 22, bottom: -14,
                            width: 1,
                            background: `linear-gradient(to bottom, ${meta.color} 0%, color-mix(in oklab, ${meta.color} 30%, transparent) 100%)`,
                            transform: "translateX(-50%)",
                          }}></div>
                        )}
                      </div>
                      <span style={{ fontSize: 13.5, lineHeight: 1.55, color: "var(--ink-2)", textWrap: "pretty", paddingTop: 2 }}>{r}</span>
                    </li>
                  );
                })}
              </ol>
            </div>

            {/* Optional inline chart */}
            {insight.data && insight.data.kind === "bars" && (
              <div style={{ marginTop: 22, padding: 18, background: "var(--surface)", borderRadius: 10, border: "1px solid var(--line)" }}>
                <div className="eyebrow" style={{ marginBottom: 10 }}>Visualized · {insight.data.labels.length} months</div>
                <BarChart values={insight.data.values} labels={insight.data.labels} color={meta.color} />
              </div>
            )}

            {/* Sources */}
            <div style={{ marginTop: 22, padding: 14, background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 8 }}>
              <div className="eyebrow" style={{ marginBottom: 10 }}>Sources · what the agent looked at</div>
              <div style={{ display: "flex", gap: 6, flexWrap: "wrap" }}>
                {sourcesFor(insight).map((s, i) => (
                  <span key={i} style={{
                    display: "inline-flex", alignItems: "center", gap: 5,
                    padding: "4px 10px",
                    background: "var(--surface-2)",
                    border: "1px solid var(--line)",
                    borderRadius: 6,
                    fontSize: 11.5,
                    fontFamily: "var(--mono)",
                    color: "var(--ink-mute)",
                  }}>
                    <span style={{ width: 4, height: 4, borderRadius: 999, background: meta.color }}></span>
                    {s}
                  </span>
                ))}
              </div>
            </div>
          </div>

          {/* Actions */}
          <div style={{ padding: "14px 26px 14px 30px", borderTop: "1px solid var(--hairline)", display: "flex", gap: 8, alignItems: "center" }}>
            {insight.actions.map((a, i) => (
              <button
                key={i}
                className={`btn ${a.primary ? "primary" : a.ghost ? "ghost" : "outline"} sm`}
                onClick={(e) => {
                  e.stopPropagation();
                  if (a.ghost || /dismiss/i.test(a.label)) onDismiss?.();
                  else onAct?.(a.label);
                }}
              >{a.label}</button>
            ))}
            <span style={{ flex: 1 }}></span>
            <span className="muted" style={{ fontSize: 11, fontFamily: "var(--mono)" }}>id: {insight.id} · {meta.label.toLowerCase()} · local-llama-3.3-8b</span>
          </div>
        </div>
      )}
    </div>
  );
}

// Map insight to data sources it consulted (mock)
function sourcesFor(insight) {
  const m = {
    pattern:      ["1,247 transactions", "14 months history", "category tags", "10 categories"],
    anomaly:      ["PG&E charges (n=18)", "12-month baseline", "category: utilities", "z-score: +3.2σ"],
    subscription: ["38 monthly charges", "subscription detection rule", "no related activity tx"],
    goal:         ["House Fund balance history", "budget envelope · Travel", "monthly contribution rates"],
    forecast:     ["account balances (6 mo)", "monthly net flow avg", "linear regression"],
    comparison:   ["category: health", "budget setting (yours)", "6-month spend avg"],
    tags:         ["transactions tagged 'business'", "attachments (none)", "$75 threshold rule"],
  };
  return m[insight.kind] || ["transaction history"];
}

function BarChart({ values, labels, color = "var(--accent)" }) {
  const W = 600, H = 120;
  const PAD = 20;
  const max = Math.max(...values) * 1.1;
  const barW = (W - PAD * 2) / values.length;
  return (
    <svg viewBox={`0 0 ${W} ${H}`} width="100%" preserveAspectRatio="xMidYMid meet">
      {values.map((v, i) => {
        const x = PAD + i * barW;
        const h = (v / max) * (H - 36);
        const y = H - 24 - h;
        return (
          <g key={i}>
            <rect x={x + 6} y={y} width={barW - 12} height={h} fill={color} opacity={i % 2 === 1 ? 1 : 0.4} rx="3" />
            <text x={x + barW / 2} y={H - 8} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-faint)" }}>{labels[i]}</text>
            <text x={x + barW / 2} y={y - 5} textAnchor="middle" style={{ fontFamily: "var(--mono)", fontSize: 11, fill: "var(--ink-mute)" }}>${v}</text>
          </g>
        );
      })}
    </svg>
  );
}

window.Insights = Insights;
