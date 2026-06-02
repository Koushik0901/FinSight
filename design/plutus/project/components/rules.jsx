function Rules() {
  const [rules, setRules] = React.useState(FS.rules);
  const [proposed, setProposed] = React.useState([
    { id: "p1", when: "Recurring", text: "Whenever a charge over $50 from a new merchant appears, hold it for 24h before categorizing." },
    { id: "p2", when: "Cash flow", text: "If Joint Checking falls below $5,000 in the next 7 days, alert me one morning before it does." },
    { id: "p3", when: "Goals",     text: "If a paycheck arrives larger than expected, sweep the surplus to the House Fund." },
    { id: "p4", when: "Tax",       text: "Tag any transaction tagged 'Home office' as deductible for next April." },
  ]);
  const toggle = (id) => {
    setRules(rs => rs.map(r => r.id === id ? { ...r, on: !r.on } : r));
    const r = rules.find(x => x.id === id);
    if (r) window.toast?.(r.on ? "Rule paused" : "Rule activated", { kind: r.on ? "warn" : "success", sub: r.lastRun });
  };
  const accept = (p) => {
    setProposed(list => list.filter(x => x.id !== p.id));
    setRules(list => [...list, {
      id: "u_" + Math.random().toString(36).slice(2, 6),
      when: ["Agent proposal", p.when, ""],
      then: [p.text, ""],
      lastRun: "new",
      on: true,
      owner: "You",
    }]);
    window.toast?.("Rule accepted", { kind: "success", sub: p.text.slice(0, 64) + (p.text.length > 64 ? "…" : "") });
  };
  const decline = (p) => {
    setProposed(list => list.filter(x => x.id !== p.id));
    window.toast?.("Proposal declined", { kind: "warn", sub: "Agent won't re-suggest this rule" });
  };

  return (
    <div className="screen">
      <SectionHeader
        eyebrow="Rules & agents"
        title="The mechanics underneath."
        action={<button className="btn" onClick={() => window.toast?.("New rule builder", { sub: "When … then … with what scope?", kind: "accent" })}><I.Plus /> New rule</button>}
      />

      <p className="muted" style={{ maxWidth: 660, marginTop: 4, marginBottom: 28, fontSize: 14, lineHeight: 1.6 }}>
        Rules are how FinSight quietly stays organized. The agent proposes them, you keep or discard. Most users never come here — but the door is always open.
      </p>

      <div style={{ display: "grid", gridTemplateColumns: "1.6fr 1fr", gap: 28 }}>
        <div>
          <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot"></span>Active · {rules.filter(r=>r.on).length} of {rules.length}</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {rules.map(r => (
              <div className="rule" key={r.id} style={{ opacity: r.on ? 1 : 0.55 }}>
                <div>
                  <div className="cond">
                    <span className="tok k">when</span>
                    {r.when.map((w, i) => <span key={i} className={`tok ${i === 1 ? "k" : ""}`}>{w}</span>)}
                    <span className="tok k">then</span>
                    {r.then.filter(Boolean).map((w, i) => <span key={i} className="tok">{w}</span>)}
                  </div>
                  <div className="muted" style={{ fontSize: 12.5, marginTop: 8, display: "flex", gap: 12, alignItems: "center" }}>
                    <span><I.Sparkle width="11" height="11" style={{ color: r.owner === "Agent" ? "var(--accent)" : "var(--ink-faint)", verticalAlign: "-1px" }} /> Owned by {r.owner}</span>
                    <span>·</span>
                    <span>{r.lastRun}</span>
                  </div>
                </div>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <span className={`tog ${r.on ? "on" : ""}`} onClick={() => toggle(r.id)}></span>
                  <button className="btn ghost sm" onClick={() => window.toast?.(r.when.join(" "), { sub: `Last: ${r.lastRun} · owner: ${r.owner}`, duration: 4500 })}><I.More /></button>
                </div>
              </div>
            ))}
          </div>

          <div className="section">
            <div className="card flush" style={{ borderColor: "var(--accent)", borderStyle: "dashed" }}>
              <div className="card-head" style={{ borderBottom: "1px dashed var(--hairline)" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <I.Sparkle style={{ color: "var(--accent)" }} />
                  <div className="h3">Agent proposals</div>
                  <span className="chip accent">{proposed.length} new</span>
                </div>
                <button className="btn ghost sm" onClick={() => window.toast?.("Why these?", { sub: "Patterns the agent saw in your last 30 days", duration: 4000 })}>Why these?</button>
              </div>
              <div style={{ padding: "4px 0" }}>
                {!proposed.length && (
                  <div style={{ padding: 32, textAlign: "center", color: "var(--ink-faint)", fontSize: 13 }}>
                    No proposals right now. Agent reviews weekly.
                  </div>
                )}
                {proposed.map(p => (
                  <div key={p.id} style={{ padding: "16px 20px", borderBottom: "1px dashed var(--hairline)", display: "grid", gridTemplateColumns: "1fr auto", gap: 14, alignItems: "center" }}>
                    <div>
                      <div className="eyebrow" style={{ marginBottom: 4 }}>{p.when}</div>
                      <div style={{ fontSize: 14, color: "var(--ink-2)", textWrap: "pretty" }}>{p.text}</div>
                    </div>
                    <div style={{ display: "flex", gap: 8 }}>
                      <button className="btn sm" onClick={() => accept(p)}>Accept</button>
                      <button className="btn ghost sm" onClick={() => decline(p)}>Decline</button>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </div>

        <div>
          <div className="card">
            <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot"></span>Agent · last 24h</div>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              <AgentLog text="Categorized 14 transactions" sub="High confidence on all but one" t="10m"/>
              <AgentLog text="Detected price change · Adobe CC" sub="$19.99 → $22.99" t="2h" />
              <AgentLog text="Moved $1,600 to House Fund" sub="Per rule: after rent posts" t="2d" />
              <AgentLog text="Re-labeled 'AMZN MKTPL' → Shopping" sub="You corrected this last month" t="3d" />
              <AgentLog text="Forecast updated for May" sub="New runway estimate: 134 days" t="3d" />
            </div>
          </div>

          <div className="card" style={{ marginTop: 16 }}>
            <div className="eyebrow" style={{ marginBottom: 10 }}><span className="dot"></span>Trust dial</div>
            <p className="muted" style={{ fontSize: 13.5, lineHeight: 1.55, marginTop: 0 }}>
              Adjust how much the agent acts without asking. You can change this per category in Settings.
            </p>
            <div style={{ marginTop: 14, padding: 12, background: "var(--surface-2)", borderRadius: 10 }}>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
                <span style={{ fontSize: 13.5 }}>Auto-categorize</span>
                <span className="chip accent">High autonomy</span>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 8 }}>
                <span style={{ fontSize: 13.5 }}>Auto-pay credit cards</span>
                <span className="chip">Confirm each time</span>
              </div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                <span style={{ fontSize: 13.5 }}>Sweep to goals</span>
                <span className="chip accent">High autonomy</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function AgentLog({ text, sub, t }) {
  return (
    <div style={{ display: "grid", gridTemplateColumns: "20px 1fr auto", gap: 10, alignItems: "start" }}>
      <span style={{ width: 8, height: 8, borderRadius: 999, background: "var(--accent)", marginTop: 6 }}></span>
      <div>
        <div style={{ fontSize: 14 }}>{text}</div>
        <div className="muted" style={{ fontSize: 12.5, marginTop: 1 }}>{sub}</div>
      </div>
      <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>{t}</span>
    </div>
  );
}

window.Rules = Rules;
