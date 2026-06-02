function Settings() {
  const [people, setPeople] = React.useState(["Mira Akinyi", "Adam Ferreira"]);
  return (
    <div className="screen">
      <SectionHeader
        eyebrow="Settings"
        title="Make it yours."
      />

      <div style={{ display: "grid", gridTemplateColumns: "200px 1fr", gap: 56, marginTop: 16 }}>
        <div style={{ position: "sticky", top: 16, alignSelf: "start", display: "flex", flexDirection: "column", gap: 2 }}>
          {["Profile", "Privacy & data", "Agent", "Appearance", "Household", "Connections", "Developer API", "Notifications", "Keyboard", "About"].map((s, i) => (
            <a key={s} href={`#sec-${i}`} className="nav-item" style={{ background: i === 1 ? "var(--surface-2)" : "transparent", color: "var(--ink-2)" }}>{s}</a>
          ))}
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 56 }}>
          <Section id="0" title="Profile" desc="Who's in this household.">
            <div className="s-row">
              <div>
                <div className="label">Household</div>
                <div className="desc">Names that show in the sidebar and reports.</div>
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
                {people.map((p, i) => (
                  <Input key={i} value={p} onChange={(v) => setPeople(ps => ps.map((x, j) => j === i ? v : x))} />
                ))}
                <button className="btn ghost sm" style={{ alignSelf: "flex-start" }} onClick={() => {
                  setPeople(ps => [...ps, "New member"]);
                  window.toast?.("Member added", { kind: "success", sub: "Click to rename" });
                }}><I.Plus /> Add person</button>
              </div>
              <div></div>
            </div>
            <div className="s-row">
              <div>
                <div className="label">Base currency</div>
                <div className="desc">Other currencies are converted at daily rates.</div>
              </div>
              <Select value="USD — US Dollar" />
              <div></div>
            </div>
          </Section>

          <Section id="1" title="Privacy & data" desc="Your money. Your machine.">
            <div className="s-row">
              <div>
                <div className="label">Local-only mode</div>
                <div className="desc">Your transactions, balances and history never leave this device. Disable to sync across your devices via end-to-end encrypted vault.</div>
              </div>
              <div style={{ fontSize: 14, color: "var(--ink-mute)" }}>Currently storing 2.4 MB on disk.</div>
              <Tog on />
            </div>
            <div className="s-row">
              <div>
                <div className="label">AI provider</div>
                <div className="desc">Categorization and forecasts can run on a local model (slower, fully private) or be sent encrypted to your chosen provider.</div>
              </div>
              <Select value="Local · Llama 3.3 (8B)" hint="No data leaves your device" />
              <div></div>
            </div>
            <div className="s-row">
              <div>
                <div className="label">Screen-share mode</div>
                <div className="desc">Replaces all amounts with bullets when you’re sharing your screen or someone walks up.</div>
              </div>
              <div style={{ fontSize: 14, color: "var(--ink-mute)" }}>Shortcut: <kbd>⌘.</kbd></div>
              <Tog />
            </div>
            <div className="s-row">
              <div>
                <div className="label">Export everything</div>
                <div className="desc">A complete CSV + JSON of your accounts, transactions, rules, and history. You own this.</div>
              </div>
              <div></div>
              <button className="btn" onClick={() => {
                const dump = { exported: new Date().toISOString(), accounts: FS.accounts.length, transactions: FS.transactions.length, goals: FS.goals.length, rules: FS.rules.length };
                const blob = new Blob([JSON.stringify(dump, null, 2)], { type: "application/json" });
                const url = URL.createObjectURL(blob);
                const a = document.createElement("a"); a.href = url; a.download = "plutus-export.json"; a.click(); URL.revokeObjectURL(url);
                window.toast?.("Full export downloaded", { kind: "success", sub: "CSV + JSON · ~2.4 MB" });
              }}>Export</button>
            </div>
          </Section>

          <Section id="2" title="Agent" desc="What the agent can do on your behalf.">
            <div className="s-row">
              <div>
                <div className="label">Auto-categorize new transactions</div>
                <div className="desc">The agent uses your past corrections and a local model to classify new activity.</div>
              </div>
              <div></div>
              <Tog on />
            </div>
            <div className="s-row">
              <div>
                <div className="label">Surface "what changed"</div>
                <div className="desc">Each morning, summarize what moved in plain language — no notifications, just a quieter Today screen.</div>
              </div>
              <div></div>
              <Tog on />
            </div>
            <div className="s-row">
              <div>
                <div className="label">Auto-pay credit cards</div>
                <div className="desc">Confirm each time before paying, or trust the rule.</div>
              </div>
              <Select value="Confirm each time" />
              <div></div>
            </div>
          </Section>

          <Section id="3" title="Appearance">
            <div className="s-row">
              <div>
                <div className="label">Theme & density</div>
                <div className="desc">These live in the Tweaks panel — open it from the toolbar to change theme, accent, and density.</div>
              </div>
              <div></div>
              <div></div>
            </div>
          </Section>

          <Section id="4" title="Household" desc="Multiple people, one private dataset.">
            <div className="s-row">
              <div>
                <div className="label">Members</div>
                <div className="desc">Each member has their own view; joint accounts roll up across the household.</div>
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
                {[
                  { name: "Mira Akinyi", role: "Co-owner", color: "var(--accent)", initial: "M" },
                  { name: "Adam Ferreira", role: "Co-owner", color: "var(--c-transport)", initial: "A" },
                ].map(m => <Member key={m.name} name={m.name} role={m.role} color={m.color} initial={m.initial} />)}
                <button className="btn outline sm" style={{ alignSelf: "flex-start" }} onClick={() => window.toast?.("Invite link generated", { kind: "success", sub: "Copied to clipboard · expires in 7 days" })}><I.Plus /> Invite member</button>
              </div>
              <div></div>
            </div>
            <div className="s-row">
              <div>
                <div className="label">Joint expense splits</div>
                <div className="desc">Optionally compute each member's share of joint expenses (50/50, by income, custom).</div>
              </div>
              <Select value="50 / 50" />
              <div></div>
            </div>
          </Section>

          <Section id="6" title="Developer API" desc="For people who want programmatic access to their own data.">
            <div className="s-row">
              <div>
                <div className="label">Local API server</div>
                <div className="desc">Run a read-only HTTP API on this device, scoped to localhost, so you can build your own dashboards, exports, or shortcuts.</div>
              </div>
              <div style={{ fontSize: 13.5, color: "var(--ink-mute)", fontFamily: "var(--mono)" }}>
                <div>http://localhost:47921</div>
                <div style={{ marginTop: 4 }}>GET /accounts · /transactions · /goals</div>
              </div>
              <Tog />
            </div>
            <div className="s-row">
              <div>
                <div className="label">Personal token</div>
                <div className="desc">Bearer token for API calls. Rotate any time.</div>
              </div>
              <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
                <code style={{ fontSize: 13, fontFamily: "var(--mono)", background: "var(--surface-2)", padding: "6px 10px", borderRadius: 6, border: "1px solid var(--line)", color: "var(--ink-mute)" }}>fs_••••••••••••••••</code>
                <button className="btn ghost sm" onClick={() => {
                  try { navigator.clipboard.writeText("fs_demo_" + Math.random().toString(36).slice(2, 18)); } catch {}
                  window.toast?.("Token copied", { kind: "success" });
                }}>Copy</button>
                <button className="btn ghost sm" onClick={() => {
                  if (confirm("Rotate API token? Any clients using the old token will need to be updated.")) {
                    window.toast?.("Token rotated", { kind: "warn", sub: "Old token revoked · update your integrations" });
                  }
                }}>Rotate</button>
              </div>
              <div></div>
            </div>
            <div className="s-row">
              <div>
                <div className="label">Webhooks</div>
                <div className="desc">POST to a URL when events happen: new transaction, anomaly detected, recurring price change, goal milestone.</div>
              </div>
              <Input value="https://example.com/finsight" />
              <button className="btn outline sm" onClick={() => {
                window.toast?.("Webhook test sent", { kind: "accent", sub: "Sample payload · awaiting 200…" });
                setTimeout(() => window.toast?.("Webhook → 200 OK", { kind: "success", sub: "Round trip 280 ms" }), 1400);
              }}>Test</button>
            </div>
            <div className="s-row">
              <div>
                <div className="label">Shortcuts &amp; Siri</div>
                <div className="desc">Hey Siri, add $14 lunch. Hey Siri, what's my runway. (Configured on the iOS app.)</div>
              </div>
              <div className="muted" style={{ fontSize: 13 }}>3 shortcuts installed</div>
              <button className="btn ghost sm" onClick={() => window.toast?.("3 shortcuts", { sub: "Quick spend · Runway · Last txn", duration: 4000 })}>Manage</button>
            </div>
          </Section>
        </div>
      </div>
    </div>
  );
}

function Section({ id, title, desc, children }) {
  return (
    <section id={`sec-${id}`}>
      <div style={{ marginBottom: 8 }}>
        <h2 className="h1" style={{ fontSize: 26 }}>{title}</h2>
        {desc && <div className="muted" style={{ fontSize: 14, marginTop: 4 }}>{desc}</div>}
      </div>
      <div>{children}</div>
    </section>
  );
}

function Input({ value, onChange }) {
  const [v, setV] = React.useState(value);
  React.useEffect(() => setV(value), [value]);
  return (
    <input value={v} onChange={(e) => { setV(e.target.value); onChange?.(e.target.value); }}
      style={{
        background: "var(--surface)",
        border: "1px solid var(--line)",
        padding: "8px 12px",
        borderRadius: 8,
        outline: "none",
        fontSize: 14,
        color: "var(--ink)",
        width: "100%",
        maxWidth: 320,
      }} />
  );
}

function Select({ value, hint }) {
  return (
    <div>
      <div style={{
        background: "var(--surface)",
        border: "1px solid var(--line)",
        padding: "8px 12px",
        borderRadius: 8,
        fontSize: 14,
        display: "flex",
        justifyContent: "space-between",
        alignItems: "center",
        gap: 10,
        maxWidth: 320,
        cursor: "pointer",
      }}>
        <span>{value}</span>
        <I.Down style={{ color: "var(--ink-faint)" }} />
      </div>
      {hint && <div className="muted" style={{ fontSize: 12.5, marginTop: 5 }}>{hint}</div>}
    </div>
  );
}

function Tog({ on: initial = false }) {
  const [on, setOn] = React.useState(initial);
  return <span className={`tog ${on ? "on" : ""}`} onClick={() => setOn(!on)}></span>;
}

function Member({ name, role, color, initial }) {
  return (
    <div style={{ display: "flex", alignItems: "center", gap: 12, padding: "8px 12px", background: "var(--surface-2)", borderRadius: 8, border: "1px solid var(--line)" }}>
      <div style={{ width: 28, height: 28, borderRadius: 999, background: color, color: "var(--accent-ink)", display: "grid", placeItems: "center", fontWeight: 600, fontSize: 13 }}>{initial}</div>
      <div style={{ flex: 1 }}>
        <div style={{ fontSize: 14, fontWeight: 500 }}>{name}</div>
        <div className="muted" style={{ fontSize: 12.5 }}>{role}</div>
      </div>
      <button className="btn ghost sm" onClick={() => window.toast?.(name, { sub: `${role} · joined Aug 2023`, duration: 3000 })}><I.More /></button>
    </div>
  );
}

window.Settings = Settings;
