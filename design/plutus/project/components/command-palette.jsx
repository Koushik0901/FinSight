function CommandPalette({ open, onClose, setRoute }) {
  const [q, setQ] = React.useState("");
  const [sel, setSel] = React.useState(0);
  const [answer, setAnswer] = React.useState(null);
  const [thinking, setThinking] = React.useState(false);
  const inputRef = React.useRef(null);

  React.useEffect(() => {
    if (open) {
      setTimeout(() => inputRef.current?.focus(), 30);
      setQ("");
      setSel(0);
      setAnswer(null);
      setThinking(false);
    }
  }, [open]);

  // Predefined ask items with embedded answers
  const asks = [
    {
      kind: "ask",
      label: "What did we spend on groceries this month vs last?",
      route: "categories",
      answer: {
        prose: "Groceries this month came to $632 across 9 transactions — $80 less than April's $712 (−11%). That's tracking with a pattern: Costco runs in alternating weeks tend to replace 2–3 smaller grocery trips. Year-to-date average is $680/mo.",
        kind: "compareBars",
        data: { current: 632, prior: 712, currentLabel: "May 2026", priorLabel: "Apr 2026", color: "#34D399" },
        actions: [
          { label: "Open Groceries category", primary: true, route: "categories" },
          { label: "See the 9 transactions", primary: false, route: "transactions" },
        ],
      },
    },
    {
      kind: "ask",
      label: "When can we afford the Italy trip?",
      route: "goals",
      answer: {
        prose: "Italy is at $1,850 of the $4,500 target. At your current $600/mo pace, you'll hit it by August 24 — about 3 weeks before the September 4 trip date. If you want to land it earlier, pulling $200/mo from the Travel envelope (which has sat unused 4 of last 5 months) gets you there by July.",
        kind: "progress",
        data: { current: 1850, target: 4500, eta: "Aug 24, 2026", color: "#C9F950" },
        actions: [
          { label: "Open Italy goal", primary: true, route: "goals" },
          { label: "Run a what-if", primary: false, route: "scenarios" },
        ],
      },
    },
    {
      kind: "ask",
      label: "Show me everything over $200 in May.",
      route: "transactions",
      answer: {
        prose: "Five transactions over $200 this month, totaling $3,022. Two were planned (rent, internet), two were variable (Costco, PG&E), one was a goal sweep. PG&E at $220 is the one outlier — already flagged as 2.1× normal.",
        kind: "list",
        data: [
          { merchant: "Bay Property · Rent", amount: 1850, date: "May 3" },
          { merchant: "PG&E", amount: 220, date: "May 10", flag: true },
          { merchant: "Costco", amount: 412, date: "May 14" },
          { merchant: "Adobe CC", amount: 540, date: "—", note: "scheduled" },
          { merchant: "House Fund sweep", amount: 1600, date: "May 3" },
        ],
        actions: [
          { label: "See in Transactions", primary: true, route: "transactions" },
        ],
      },
    },
    {
      kind: "ask",
      label: "What subscriptions have I not used in 90 days?",
      route: "recurring",
      answer: {
        prose: "Two: Disney+ (0 plays in 90 days, $10.99/mo, $132/yr) and MasterClass (free trial ending in 5 days, will start at $180/yr if not cancelled). Together that's $312/yr you can keep.",
        kind: "list",
        data: [
          { merchant: "Disney+", amount: 10.99, date: "monthly", flag: true, note: "0 plays in 90d" },
          { merchant: "MasterClass", amount: 15, date: "monthly", flag: true, note: "trial ends May 26" },
        ],
        actions: [
          { label: "Open subscriptions", primary: true, route: "recurring" },
          { label: "Draft cancellations", primary: false, route: "recurring" },
        ],
      },
    },
    {
      kind: "ask",
      label: "How does my dining compare to people like me?",
      route: "reports",
      answer: {
        prose: "FinSight doesn't compare you to others — your data never leaves this machine and we don't aggregate it. But here's a comparison that's actually useful: your dining vs your own 12-month average. May ($412) is +13% over the $365 monthly average. Three of the last four months landed within $50 of average.",
        kind: "compareBars",
        data: { current: 412, prior: 365, currentLabel: "May 2026", priorLabel: "12-mo avg", color: "#FB923C" },
        actions: [
          { label: "See dining trend", primary: true, route: "reports" },
        ],
      },
    },
    {
      kind: "ask",
      label: "What's our savings rate this month?",
      route: "reports",
      answer: {
        prose: "May savings rate is 28%, the highest month of the year so far. Year-to-date average is 19%. The boost is mostly from Travel sitting at $0 — pull that out and underlying rate is more like 23%.",
        kind: "bigNumber",
        data: { value: "28%", sub: "of $10,000 income kept", color: "#C9F950" },
        actions: [
          { label: "Open Reports", primary: true, route: "reports" },
        ],
      },
    },
  ];

  const all = React.useMemo(() => {
    const nav = [
      { kind: "nav", id: "today",        label: "Go to Today",        ico: I.Today },
      { kind: "nav", id: "accounts",     label: "Go to Accounts",     ico: I.Wallet },
      { kind: "nav", id: "transactions", label: "Go to Transactions", ico: I.Flow },
      { kind: "nav", id: "budget",       label: "Go to Budget",       ico: I.Lego },
      { kind: "nav", id: "categories",   label: "Go to Categories",   ico: I.Grid },
      { kind: "nav", id: "recurring",    label: "Go to Recurring",    ico: I.Repeat },
      { kind: "nav", id: "goals",        label: "Go to Goals",        ico: I.Goal },
      { kind: "nav", id: "reports",      label: "Go to Reports",      ico: I.Spark },
      { kind: "nav", id: "insights",     label: "Go to Insights",     ico: I.Sparkle },
      { kind: "nav", id: "scenarios",    label: "Go to Scenarios",    ico: I.Bolt },
      { kind: "nav", id: "rules",        label: "Go to Rules",        ico: I.Bolt },
      { kind: "nav", id: "settings",     label: "Go to Settings",     ico: I.Gear },
    ];
    const actions = [
      { kind: "act", label: "Add a transaction…",       ico: I.Plus, hint: "manual" },
      { kind: "act", label: "Toggle privacy mode",      ico: I.EyeOff, hint: "⌘." },
      { kind: "act", label: "Plan next month",          ico: I.Sparkle, route: "budget" },
      { kind: "act", label: "Run a what-if",            ico: I.Bolt, route: "scenarios" },
      { kind: "act", label: "Export this month as CSV", ico: I.ArrowDown },
    ];
    return [...asks, ...nav, ...actions];
  }, []);

  const filtered = React.useMemo(() => {
    if (!q.trim()) return all;
    const s = q.toLowerCase();
    return all.filter(x => x.label.toLowerCase().includes(s));
  }, [q, all]);

  React.useEffect(() => { setSel(0); }, [q]);

  React.useEffect(() => {
    if (!open) return;
    const onKey = (e) => {
      if (e.key === "Escape") {
        if (answer) setAnswer(null);
        else onClose();
      }
      else if (e.key === "ArrowDown" && !answer) { e.preventDefault(); setSel(s => Math.min(filtered.length - 1, s + 1)); }
      else if (e.key === "ArrowUp" && !answer)   { e.preventDefault(); setSel(s => Math.max(0, s - 1)); }
      else if (e.key === "Enter" && !answer) {
        const it = filtered[sel];
        if (it) handle(it);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, filtered, sel, answer]);

  const handle = (it) => {
    if (it.kind === "nav") { setRoute(it.id); onClose(); }
    else if (it.kind === "ask") {
      setThinking(true);
      setAnswer(it.answer);
      setTimeout(() => setThinking(false), 600);
    }
    else if (it.route) { setRoute(it.route); onClose(); }
    else { onClose(); }
  };

  // Custom ask: if user types a question and hits enter
  const submitCustom = () => {
    if (!q.trim()) return;
    setThinking(true);
    setAnswer({
      prose: `The agent is thinking about "${q.trim()}". In a working build, this would generate a grounded answer with charts from your real data. For now, try one of the example questions on the left.`,
      kind: "bigNumber",
      data: { value: "—", sub: "answer pending", color: "var(--ink-mute)" },
      actions: [
        { label: "Try a different question", primary: false },
      ],
    });
    setTimeout(() => setThinking(false), 800);
  };

  if (!open) return null;

  const asksF = filtered.filter(x => x.kind === "ask");
  const navsF = filtered.filter(x => x.kind === "nav");
  const actsF = filtered.filter(x => x.kind === "act");
  let idx = 0;
  const renderItems = (list) => list.map(it => {
    const myIdx = idx++;
    const isSel = myIdx === sel;
    const Ico = it.ico;
    return (
      <div key={myIdx} className={`cmdk-item ${isSel ? "sel" : ""}`}
        onMouseEnter={() => setSel(myIdx)}
        onClick={() => handle(it)}>
        {it.kind === "ask" ? <I.Sparkle className="ico" style={{ color: "var(--accent)" }} /> : (Ico ? <Ico className="ico" /> : null)}
        <span>{it.label}</span>
        {it.hint && <span className="hint">{it.hint}</span>}
      </div>
    );
  });

  return (
    <div className="cmdk-mask" onClick={onClose}>
      <div className="cmdk" onClick={(e) => e.stopPropagation()} style={{ width: answer ? "min(760px, 94vw)" : "min(620px, 92vw)" }}>
        <div className="cmdk-input">
          {answer ? (
            <I.Sparkle style={{ color: "var(--accent)" }} />
          ) : (
            <I.Search style={{ color: "var(--ink-mute)" }} />
          )}
          <input ref={inputRef} value={q} onChange={e => setQ(e.target.value)}
                 onKeyDown={e => {
                   if (e.key === "Enter" && q.trim() && !filtered.length) {
                     e.preventDefault(); submitCustom();
                   }
                 }}
                 placeholder={answer ? "Ask a follow-up…" : "Search, ask anything, or jump…"} />
          {answer && <button className="btn ghost sm" onClick={() => setAnswer(null)}>Back</button>}
          <span className="kbd" style={{ fontFamily: "var(--mono)" }}>esc</span>
        </div>

        {/* Answer mode */}
        {answer ? (
          <AnswerPanel answer={answer} thinking={thinking} setRoute={setRoute} onClose={onClose} />
        ) : (
          <div className="cmdk-list">
            {asksF.length > 0 && <><div className="cmdk-section">Ask the agent</div>{renderItems(asksF)}</>}
            {navsF.length > 0 && <><div className="cmdk-section">Jump to</div>{renderItems(navsF)}</>}
            {actsF.length > 0 && <><div className="cmdk-section">Actions</div>{renderItems(actsF)}</>}
            {filtered.length === 0 && (
              <div className="cmdk-item" style={{ color: "var(--ink-mute)", padding: 14 }} onClick={submitCustom}>
                <I.Sparkle className="ico" style={{ color: "var(--accent)" }} />
                <div style={{ flex: 1 }}>
                  <div>Ask the agent: "{q}"</div>
                  <div className="muted" style={{ fontSize: 11.5, marginTop: 4 }}>The agent reads your full data and answers in plain language.</div>
                </div>
                <span className="hint">⏎</span>
              </div>
            )}
          </div>
        )}

        {!answer && (
          <div className="cmdk-foot">
            <span><span className="k">↑↓</span> navigate</span>
            <span><span className="k">↵</span> select</span>
            <span><span className="k">esc</span> close</span>
          </div>
        )}
      </div>
    </div>
  );
}

function AnswerPanel({ answer, thinking, setRoute, onClose }) {
  if (thinking) {
    return (
      <div style={{ padding: 36, textAlign: "center" }}>
        <div style={{ display: "inline-flex", alignItems: "center", gap: 12 }}>
          <span style={{ width: 14, height: 14, borderRadius: 999, background: "var(--accent)", animation: "pulse 1.4s infinite" }}></span>
          <span style={{ fontSize: 14, color: "var(--ink-2)" }}>Reading your data…</span>
        </div>
      </div>
    );
  }

  return (
    <div style={{ padding: 24 }}>
      <div style={{ fontSize: 15, lineHeight: 1.6, color: "var(--ink)", textWrap: "pretty" }}>
        {answer.prose}
      </div>

      {answer.kind === "compareBars" && (
        <div style={{ marginTop: 20, padding: 16, background: "var(--surface-2)", borderRadius: 10, border: "1px solid var(--line)" }}>
          <CompareBars data={answer.data} />
        </div>
      )}
      {answer.kind === "progress" && (
        <div style={{ marginTop: 20, padding: 16, background: "var(--surface-2)", borderRadius: 10, border: "1px solid var(--line)" }}>
          <ProgressViz data={answer.data} />
        </div>
      )}
      {answer.kind === "list" && (
        <div style={{ marginTop: 20, padding: 4, background: "var(--surface-2)", borderRadius: 10, border: "1px solid var(--line)" }}>
          {answer.data.map((row, i) => (
            <div key={i} style={{ display: "grid", gridTemplateColumns: "70px 1fr auto", gap: 14, padding: "10px 14px", borderBottom: i === answer.data.length - 1 ? "0" : "1px solid var(--hairline)", alignItems: "center" }}>
              <span className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)" }}>{row.date}</span>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ fontSize: 13.5 }}>{row.merchant}</span>
                {row.flag && <span className="chip warning" style={{ padding: "1px 7px", fontSize: 10 }}>{row.note || "flag"}</span>}
                {!row.flag && row.note && <span className="muted" style={{ fontSize: 11.5 }}>· {row.note}</span>}
              </div>
              <span className="num tabular" style={{ fontSize: 14, fontFamily: "var(--mono)" }}>${row.amount.toLocaleString()}</span>
            </div>
          ))}
        </div>
      )}
      {answer.kind === "bigNumber" && (
        <div style={{ marginTop: 20, padding: 28, background: "var(--surface-2)", borderRadius: 10, border: "1px solid var(--line)", textAlign: "center" }}>
          <div className="figure" style={{ fontSize: 72, lineHeight: 1, color: answer.data.color || "var(--accent)", letterSpacing: "-0.04em" }}>{answer.data.value}</div>
          <div className="muted" style={{ fontSize: 13, marginTop: 8 }}>{answer.data.sub}</div>
        </div>
      )}

      {answer.actions && (
        <div style={{ marginTop: 20, display: "flex", gap: 8, flexWrap: "wrap" }}>
          {answer.actions.map((a, i) => (
            <button key={i} className={`btn ${a.primary ? "primary" : "outline"} sm`}
              onClick={() => { if (a.route) { setRoute(a.route); onClose(); } }}>
              {a.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function CompareBars({ data }) {
  const max = Math.max(data.current, data.prior) * 1.1;
  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 6 }}>
        <span style={{ fontSize: 12.5 }}>{data.priorLabel}</span>
        <span className="num tabular muted" style={{ fontSize: 12.5 }}>${data.prior.toLocaleString()}</span>
      </div>
      <div style={{ height: 14, background: "var(--surface)", borderRadius: 4, overflow: "hidden" }}>
        <div style={{ width: `${(data.prior / max) * 100}%`, height: "100%", background: "var(--ink-faint)", opacity: 0.5 }} />
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 16, marginBottom: 6 }}>
        <span style={{ fontSize: 12.5 }}>{data.currentLabel}</span>
        <span className="num tabular" style={{ fontSize: 12.5, color: data.color }}>${data.current.toLocaleString()}</span>
      </div>
      <div style={{ height: 14, background: "var(--surface)", borderRadius: 4, overflow: "hidden" }}>
        <div style={{ width: `${(data.current / max) * 100}%`, height: "100%", background: data.color, boxShadow: `0 0 12px ${data.color}66` }} />
      </div>
      <div style={{ marginTop: 12, fontSize: 12, color: "var(--ink-mute)", fontFamily: "var(--mono)" }}>
        {data.current < data.prior
          ? `↓ $${(data.prior - data.current).toLocaleString()} (${Math.round(((data.prior - data.current) / data.prior) * 100)}% less)`
          : `↑ $${(data.current - data.prior).toLocaleString()} (${Math.round(((data.current - data.prior) / data.prior) * 100)}% more)`}
      </div>
    </div>
  );
}

function ProgressViz({ data }) {
  const pct = (data.current / data.target) * 100;
  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
        <span className="figure" style={{ fontSize: 24, color: data.color }}>${data.current.toLocaleString()}</span>
        <span className="muted" style={{ fontSize: 13 }}>of ${data.target.toLocaleString()}</span>
      </div>
      <div style={{ height: 10, background: "var(--surface)", borderRadius: 999, overflow: "hidden" }}>
        <div style={{ width: pct + "%", height: "100%", background: data.color, borderRadius: 999, boxShadow: `0 0 12px ${data.color}66` }} />
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", marginTop: 10, fontSize: 12, color: "var(--ink-mute)", fontFamily: "var(--mono)" }}>
        <span>{Math.round(pct)}% there</span>
        <span>ETA {data.eta}</span>
      </div>
    </div>
  );
}

window.CommandPalette = CommandPalette;
