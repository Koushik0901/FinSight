function Onboarding({ onDone }) {
  const [step, setStep] = React.useState(0);
  const [selBank, setSelBank] = React.useState(null);
  const [linked, setLinked] = React.useState(false);
  const [linking, setLinking] = React.useState(false);
  const [goalAmt, setGoalAmt] = React.useState(80000);
  const [goalDate, setGoalDate] = React.useState("Mar 2027");
  const [trustLevel, setTrustLevel] = React.useState(2);

  const steps = ["Welcome", "Concept", "Link an account", "Watch it work", "Pick categories", "A first goal", "Trust the agent"];

  const next = () => setStep(s => Math.min(steps.length - 1, s + 1));
  const back = () => setStep(s => Math.max(0, s - 1));

  // Auto-advance the "linking" simulation
  React.useEffect(() => {
    if (step === 1 && linking) {
      const t = setTimeout(() => { setLinking(false); setLinked(true); }, 1600);
      return () => clearTimeout(t);
    }
  }, [step, linking]);

  return (
    <div className="onb-shell">
      <div className="onb-top">
        <div className="brand" style={{ padding: 0 }}>
          <div className="mark"></div>
          <div className="wm">FinSight</div>
        </div>
        <div className="onb-steps">
          {steps.map((_, i) => (
            <div key={i} className={`onb-step-pip ${i < step ? "done" : ""} ${i === step ? "cur" : ""}`}></div>
          ))}
        </div>
        <button className="btn ghost sm" onClick={onDone}>Skip setup</button>
      </div>

      <div className="onb-body">
        {step === 0 && <Welcome onPractice={() => { setStep(steps.length - 1); }} />}
        {step === 1 && <Concept />}
        {step === 2 && <LinkAccount sel={selBank} setSel={setSelBank} linking={linking} linked={linked} startLink={() => setLinking(true)} />}
        {step === 3 && <Watch />}
        {step === 4 && <PickCategories />}
        {step === 5 && <FirstGoal amt={goalAmt} setAmt={setGoalAmt} date={goalDate} setDate={setGoalDate} />}
        {step === 6 && <Trust level={trustLevel} setLevel={setTrustLevel} />}
      </div>

      <div className="onb-foot">
        <button className="btn ghost" onClick={back} disabled={step === 0} style={{ opacity: step === 0 ? 0.4 : 1 }}>
          <I.ArrowL /> Back
        </button>
        <div className="muted" style={{ fontSize: 13 }}>
          Step <span className="mono">{step + 1}</span> of {steps.length} · <span className="mono">{steps[step]}</span>
        </div>
        {step === steps.length - 1 ? (
          <button className="btn primary" onClick={onDone}>Open my dashboard <I.ArrowR /></button>
        ) : (
          <button
            className="btn primary"
            onClick={next}
            disabled={step === 2 && !linked}
            style={{ opacity: step === 2 && !linked ? 0.5 : 1, cursor: step === 2 && !linked ? "not-allowed" : "pointer" }}
          >
            Continue <I.ArrowR />
          </button>
        )}
      </div>
    </div>
  );
}

function Welcome({ onPractice }) {
  return (
    <>
      <div className="onb-left">
        <div className="num-step">001 · Welcome</div>
        <h1>This is a calm place for your money.</h1>
        <p className="lead">
          FinSight reads your accounts, organizes the noise, and shows you what changed. No bright colors, no nudges, no streaks — just an accurate picture you can open in the morning the way someone might check the weather.
        </p>
        <div style={{ display: "flex", gap: 10, marginBottom: 28, flexWrap: "wrap" }}>
          <span className="chip"><I.Lock width="11" height="11" /> Local-first</span>
          <span className="chip">Open source</span>
          <span className="chip">No ads, ever</span>
          <span className="chip accent"><span className="dot"></span>Free forever</span>
        </div>
        <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
          <button className="btn outline" onClick={onPractice}>Try with sample data first</button>
          <span className="muted" style={{ fontSize: 13 }}>Explore Mira &amp; Adam's household — nothing to link.</span>
        </div>
      </div>
      <div className="onb-right">
        <WelcomeArt />
      </div>
    </>
  );
}

function Concept() {
  const ideas = [
    { n: "01", title: "Show the signal, not the verdict.", body: "We surface numbers and trends. You decide what they mean. The app never moralizes about spending." },
    { n: "02", title: "Runway is the most honest number.", body: "Not balance. Not budget. How many days you'd last at your current pace. The hero of every screen." },
    { n: "03", title: "The agent acts; you stay in charge.", body: "It categorizes, audits, drafts cancellations. It pauses before anything that moves money — your hand on the button." },
  ];
  return (
    <>
      <div className="onb-left">
        <div className="num-step">002 · How to think about this</div>
        <h1>A few ideas, before the buttons.</h1>
        <p className="lead">
          The interface follows from a small worldview. You don't need to memorize it — just know it exists, and you'll find the app behaves predictably.
        </p>
      </div>
      <div className="onb-right">
        <div style={{ display: "flex", flexDirection: "column", gap: 14, width: "100%", maxWidth: 480 }}>
          {ideas.map(i => (
            <div key={i.n} className="card" style={{ padding: 22 }}>
              <div className="eyebrow" style={{ marginBottom: 8 }}>{i.n}</div>
              <div style={{ fontSize: 18, fontWeight: 500, letterSpacing: "-0.02em", marginBottom: 6 }}>{i.title}</div>
              <div className="muted" style={{ fontSize: 14, lineHeight: 1.55 }}>{i.body}</div>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}

function PickCategories() {
  const recipes = [
    { id: "renting",   name: "Renting in a city", icon: "🏙", desc: "Rent, utilities, groceries, dining, transit, subscriptions", cats: 12 },
    { id: "owner",     name: "Homeowner",         icon: "🏠", desc: "Mortgage, property tax, insurance, repairs, utilities…", cats: 16 },
    { id: "freelance", name: "Freelancer · irregular income", icon: "💼", desc: "Plus business expenses, quarterly tax setaside", cats: 18 },
    { id: "family",    name: "Couple with a kid", icon: "👶", desc: "Childcare, school, family medical, family travel", cats: 17 },
  ];
  const [sel, setSel] = React.useState("renting");
  return (
    <>
      <div className="onb-left">
        <div className="num-step">005 · Categories to start with</div>
        <h1>Pick a recipe.<br/>Customize anything later.</h1>
        <p className="lead">
          Most categorization apps make you build categories from scratch. We start you with a sensible set for your life stage — it'll match how the agent already classified your transactions.
        </p>
        <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 460 }}>
          {recipes.map(r => (
            <div key={r.id} className={`prov ${sel === r.id ? "sel" : ""}`} onClick={() => setSel(r.id)} style={{ padding: 14 }}>
              <div className="logo" style={{ background: "var(--surface-2)", fontSize: 16, color: "var(--ink)" }}>{r.icon}</div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 14, fontWeight: 500 }}>{r.name}</div>
                <div className="muted" style={{ fontSize: 13, marginTop: 2 }}>{r.cats} categories · {r.desc}</div>
              </div>
              {sel === r.id && <I.Check style={{ color: "var(--accent)" }} />}
            </div>
          ))}
        </div>
      </div>
      <div className="onb-right">
        <div className="card" style={{ width: "100%", maxWidth: 460, padding: 0, overflow: "hidden" }}>
          <div className="card-head"><div className="h3">Preview · {recipes.find(r => r.id === sel)?.name}</div></div>
          <div style={{ padding: 14, display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 6 }}>
            {[
              { c: FS.categories[0], note: "you set the budget"},
              { c: FS.categories[1], note: "" },
              { c: FS.categories[2], note: "" },
              { c: FS.categories[3], note: "" },
              { c: FS.categories[4], note: "" },
              { c: FS.categories[5], note: "auto-detected" },
              { c: FS.categories[6], note: "" },
              { c: FS.categories[7], note: "" },
              { c: FS.categories[8], note: "" },
              { c: FS.categories[9], note: "" },
            ].map((row, i) => (
              <div key={i} style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 10px", borderRadius: 7, background: "var(--surface-2)" }}>
                <span className="cswatch" style={{ background: row.c.color }}></span>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 13.5 }}>{row.c.label}</div>
                  {row.note && <div className="muted" style={{ fontSize: 11.5 }}>{row.note}</div>}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}

function WelcomeArt() {
  return (
    <div style={{ width: "100%", display: "flex", flexDirection: "column", gap: 14 }}>
      <div className="card" style={{ background: "var(--surface)", padding: 28 }}>
        <div className="eyebrow"><span className="dot"></span>Sunday, May 18</div>
        <h2 className="h1" style={{ fontSize: 30, marginTop: 14, lineHeight: 1.15, fontWeight: 500, color: "var(--ink-mute)" }}>
          You have <span className="figure" style={{ color: "var(--accent)", fontWeight: 600 }}>$48,920</span> across <span style={{ color: "var(--ink)" }}>six accounts</span>.
        </h2>
        <p className="muted" style={{ fontSize: 14, marginTop: 14, lineHeight: 1.55 }}>
          You're tracking 11% below April. The agent flagged one subscription you haven't used in 90 days.
        </p>
      </div>
      <div style={{ display: "flex", gap: 12 }}>
        <div className="card tight" style={{ flex: 1 }}>
          <div className="eyebrow">Runway</div>
          <div className="figure" style={{ fontSize: 26, marginTop: 6, letterSpacing: "-0.025em" }}>134 <span className="muted" style={{ fontSize: 14, fontWeight: 500 }}>days</span></div>
        </div>
        <div className="card tight" style={{ flex: 1 }}>
          <div className="eyebrow">Recurring</div>
          <div className="figure" style={{ fontSize: 26, marginTop: 6, letterSpacing: "-0.025em" }}>$2,584<span className="muted" style={{ fontSize: 14, fontWeight: 500 }}>/mo</span></div>
        </div>
      </div>
    </div>
  );
}

function LinkAccount({ sel, setSel, linking, linked, startLink }) {
  const banks = [
    { id: "chase",     name: "Chase",        color: "oklch(0.55 0.10 220)" },
    { id: "schwab",    name: "Schwab",       color: "oklch(0.55 0.10 250)" },
    { id: "mercury",   name: "Mercury",      color: "oklch(0.55 0.10 60)" },
    { id: "amex",      name: "Amex",         color: "oklch(0.55 0.10 200)" },
    { id: "wf",        name: "Wealthfront",  color: "oklch(0.55 0.08 145)" },
    { id: "fidelity",  name: "Fidelity",     color: "oklch(0.55 0.08 90)" },
  ];
  return (
    <>
      <div className="onb-left">
        <div className="num-step">002 · Link your first account</div>
        <h1>Connect a bank.<br/>You can add the rest later.</h1>
        <p className="lead">
          We use read-only credentials and tokens scoped to your device. Disconnect at any time and the data goes with it.
        </p>
        <div className="prov-grid" style={{ maxWidth: 420 }}>
          {banks.map(b => (
            <div key={b.id} className={`prov ${sel === b.id ? "sel" : ""}`} onClick={() => setSel(b.id)}>
              <div className="logo" style={{ background: b.color, color: "white" }}>{b.name[0]}</div>
              <div style={{ flex: 1, fontSize: 14 }}>{b.name}</div>
              {sel === b.id && <I.Check style={{ color: "var(--accent)" }} />}
            </div>
          ))}
        </div>
        <div style={{ marginTop: 18, display: "flex", gap: 10, alignItems: "center" }}>
          <button className="btn primary" onClick={startLink} disabled={!sel || linking || linked} style={{ opacity: (!sel || linking) ? 0.5 : 1 }}>
            {linking ? "Connecting…" : linked ? "Connected ✓" : `Connect via ${banks.find(b => b.id === sel)?.name || "—"}`}
          </button>
          <span className="muted" style={{ fontSize: 13 }}>Or <a style={{ color: "var(--accent)", textDecoration: "underline" }}>import a CSV</a></span>
        </div>
      </div>
      <div className="onb-right">
        <ConnectAnim linking={linking} linked={linked} />
      </div>
    </>
  );
}

function ConnectAnim({ linking, linked }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: 12, width: "100%", maxWidth: 480 }}>
      <div className="card" style={{ padding: 22 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
          <div style={{ width: 28, height: 28, borderRadius: 8, background: "var(--accent-2)", display: "grid", placeItems: "center", color: "var(--accent)" }}>
            <I.Lock />
          </div>
          <div>
            <div style={{ fontSize: 14, fontWeight: 500 }}>Secure connection</div>
            <div className="muted" style={{ fontSize: 13 }}>Credentials encrypted at rest on this device.</div>
          </div>
        </div>
      </div>
      <div className="card" style={{ padding: 20 }}>
        {[
          "Establishing read-only session",
          "Verifying account ownership",
          "Importing the last 24 months",
          "Local agent categorizing transactions",
        ].map((s, i) => {
          const done = linked || (linking && i < 2);
          const cur = linking && !linked && i === 2;
          return (
            <div key={i} style={{ display: "grid", gridTemplateColumns: "20px 1fr auto", gap: 10, alignItems: "center", padding: "8px 0" }}>
              <span style={{
                width: 14, height: 14, borderRadius: 999,
                border: "1.5px solid " + (done ? "var(--accent)" : cur ? "var(--accent)" : "var(--line-2)"),
                background: done ? "var(--accent)" : "transparent",
                display: "grid", placeItems: "center",
              }}>
                {done && <I.Check width="9" height="9" style={{ color: "var(--surface)" }} />}
                {cur && <span style={{ width: 5, height: 5, borderRadius: 999, background: "var(--accent)", animation: "pulse 1.4s infinite" }}></span>}
              </span>
              <span style={{ fontSize: 14, color: done ? "var(--ink)" : cur ? "var(--ink)" : "var(--ink-faint)" }}>{s}</span>
              {done && <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)" }}>ok</span>}
            </div>
          );
        })}
      </div>
      {linked && (
        <div className="card" style={{ background: "var(--accent-2)", borderColor: "var(--accent-3)" }}>
          <div className="strong" style={{ fontSize: 14 }}>Connected · 1,247 transactions imported</div>
          <div className="muted" style={{ fontSize: 13, marginTop: 4 }}>Categorized 1,231 with high confidence. 16 need a glance — we’ll handle them on the next screen.</div>
        </div>
      )}
    </div>
  );
}

function Watch() {
  // animated counter
  const [n, setN] = React.useState(0);
  React.useEffect(() => {
    const start = Date.now();
    const t = setInterval(() => {
      const e = (Date.now() - start) / 1200;
      if (e >= 1) { setN(1247); clearInterval(t); }
      else setN(Math.floor(1247 * e));
    }, 30);
    return () => clearInterval(t);
  }, []);

  return (
    <>
      <div className="onb-left">
        <div className="num-step">003 · The agent went to work</div>
        <h1>While you read this, it organized everything.</h1>
        <p className="lead">
          A local model read your imported history and labeled each transaction with a category, a merchant, and a confidence score. You can correct anything — the agent learns from you, never the other way around.
        </p>
        <div className="card" style={{ padding: 20, marginTop: 16, maxWidth: 460 }}>
          <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline" }}>
            <div className="figure" style={{ fontSize: 44, lineHeight: 1 }}>{n.toLocaleString()}</div>
            <div className="num pos" style={{ fontSize: 13 }}>✓ done</div>
          </div>
          <div className="muted" style={{ fontSize: 13.5, marginTop: 4 }}>transactions across 24 months categorized</div>
          <div style={{ display: "flex", gap: 8, marginTop: 14 }}>
            <span className="chip positive">98.7% high confidence</span>
            <span className="chip">16 to review</span>
          </div>
        </div>
      </div>
      <div className="onb-right">
        <div style={{ width: "100%", maxWidth: 460 }}>
          <div className="card flush">
            <div className="card-head"><div className="h3">Sample of what it did</div></div>
            <table className="tbl">
              <tbody>
                {[
                  ["May 14", "Costco",       "Groceries",  -412.00, "merchant model"],
                  ["May 13", "Whole Foods",  "Groceries",   -64.30, ""],
                  ["May 12", "BP Gas",       "Transport",   -52.40, ""],
                  ["May 10", "PG&E",         "Utilities",  -220.00, "recurring detected"],
                  ["May 09", "Walgreens",    "Health",      -32.10, ""],
                  ["May 08", "Sonic",        "Utilities",   -88.00, "recurring detected"],
                  ["May 05", "Trader Joe's", "Groceries",   -52.40, ""],
                ].map(([d, m, c, a, hint], i) => (
                  <tr key={i}>
                    <td style={{ width: 60, color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 12.5 }}>{d}</td>
                    <td>
                      <div style={{ fontSize: 14 }}>{m}</div>
                      {hint && <div style={{ fontSize: 12, color: "var(--accent)", marginTop: 1 }}>✦ {hint}</div>}
                    </td>
                    <td className="muted" style={{ fontSize: 13 }}>{c}</td>
                    <td className="right num tabular">{FS.fmt(a, { decimals: 2 })}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </>
  );
}

function FirstGoal({ amt, setAmt, date, setDate }) {
  const presets = [
    { name: "House down payment", icon: "🏡", amount: 80000, date: "Mar 2027" },
    { name: "Emergency fund",     icon: "🛟", amount: 24000, date: "Sep 2026" },
    { name: "A trip",             icon: "✈", amount: 4500,  date: "Aug 2026" },
    { name: "Pay off a card",     icon: "💳", amount: 2418,  date: "Jul 2026" },
  ];

  return (
    <>
      <div className="onb-left">
        <div className="num-step">004 · One goal to start</div>
        <h1>What are you moving toward?</h1>
        <p className="lead">
          You can add more later. Goals are horizon lines — the agent gently moves money toward each one on a cadence you set. Nothing happens without you knowing.
        </p>
        <div style={{ display: "grid", gridTemplateColumns: "repeat(2, 1fr)", gap: 10, maxWidth: 440, marginTop: 6 }}>
          {presets.map(p => (
            <div key={p.name} className="prov" style={{ padding: "12px 14px" }} onClick={() => { setAmt(p.amount); setDate(p.date); }}>
              <div className="logo" style={{ background: "var(--surface-2)", fontSize: 18 }}>{p.icon}</div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 14 }}>{p.name}</div>
                <div className="muted" style={{ fontSize: 12.5, marginTop: 1 }}>{FS.fmt(p.amount)} · by {p.date}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
      <div className="onb-right">
        <div className="card" style={{ width: "100%", maxWidth: 440 }}>
          <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot"></span>Your goal</div>
          <div className="h1" style={{ fontSize: 28, lineHeight: 1.2 }}>Save <span className="figure" style={{ color: "var(--accent)" }}>{FS.fmt(amt)}</span> by <span className="figure" style={{ color: "var(--accent)" }}>{date}</span>.</div>
          <div className="muted" style={{ fontSize: 14, marginTop: 14, lineHeight: 1.5 }}>
            That’s about <span className="strong">{FS.fmt(Math.round(amt / 10))}/mo</span> moved automatically from your joint checking to a high-yield savings account after each paycheck.
          </div>
          <div style={{ marginTop: 20, display: "flex", gap: 8 }}>
            <button className="btn">Adjust pace</button>
            <button className="btn ghost">Choose account</button>
          </div>
          <div style={{ marginTop: 24, padding: 14, background: "var(--surface-2)", borderRadius: 10 }}>
            <div className="eyebrow" style={{ marginBottom: 6 }}>Preview</div>
            <div style={{ display: "flex", alignItems: "baseline", justifyContent: "space-between" }}>
              <div className="figure" style={{ fontSize: 18 }}>$0 today</div>
              <div className="figure" style={{ fontSize: 18, color: "var(--accent)" }}>{FS.fmt(amt)} on {date}</div>
            </div>
            <div className="goal-bar" style={{ marginTop: 8 }}><span style={{ width: "3%" }}></span></div>
          </div>
        </div>
      </div>
    </>
  );
}

function Trust({ level, setLevel }) {
  const levels = [
    { name: "Cautious",   desc: "Confirm every meaningful action. The agent never moves money on its own." },
    { name: "Balanced",   desc: "Categorize and audit silently. Ask before paying bills or moving funds." },
    { name: "High autonomy", desc: "Categorize, audit, pay recurring bills from chosen accounts, and rebalance toward goals." },
  ];
  return (
    <>
      <div className="onb-left">
        <div className="num-step">005 · Set the trust dial</div>
        <h1>How much should the agent do on its own?</h1>
        <p className="lead">
          You can change this any time. Per-category overrides live in Settings — for example, you can fully trust grocery categorization while keeping a tight rein on transfers.
        </p>
        <div style={{ display: "flex", flexDirection: "column", gap: 8, maxWidth: 480, marginTop: 4 }}>
          {levels.map((l, i) => (
            <div key={l.name} className={`prov ${level === i ? "sel" : ""}`} onClick={() => setLevel(i)} style={{ alignItems: "flex-start", padding: 14 }}>
              <div className="logo" style={{ background: level === i ? "var(--accent)" : "var(--surface-2)", color: level === i ? "var(--surface)" : "var(--ink-mute)", fontSize: 14, fontFamily: "var(--sans)", fontWeight: 600 }}>{i + 1}</div>
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 14, fontWeight: 500 }}>{l.name}</div>
                <div className="muted" style={{ fontSize: 13.5, marginTop: 4, lineHeight: 1.45 }}>{l.desc}</div>
              </div>
            </div>
          ))}
        </div>
      </div>
      <div className="onb-right">
        <div className="card" style={{ width: "100%", maxWidth: 460 }}>
          <div className="eyebrow" style={{ marginBottom: 12 }}><span className="dot"></span>You’re all set</div>
          <div className="h1" style={{ fontSize: 28, lineHeight: 1.15 }}>
            Tomorrow morning, FinSight will know your money better than your bank does.
          </div>
          <p className="muted" style={{ fontSize: 14, marginTop: 14, lineHeight: 1.55 }}>
            We’ll prepare a quiet briefing for you each morning. No notifications, no badges. If something genuinely needs your attention, you’ll see a single line of text on the Today screen.
          </p>
          <div style={{ marginTop: 20, display: "flex", gap: 8, flexWrap: "wrap" }}>
            <span className="chip">⌘K opens the palette anywhere</span>
            <span className="chip">⌘. hides amounts</span>
            <span className="chip">⌘← / ⌘→ moves through months</span>
          </div>
        </div>
      </div>
    </>
  );
}

window.Onboarding = Onboarding;
