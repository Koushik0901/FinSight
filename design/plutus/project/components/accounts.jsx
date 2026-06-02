function Accounts({ setRoute }) {
  const [selected, setSelected] = React.useState(FS.accounts[0].id);
  const acct = FS.accounts.find(a => a.id === selected);
  const txnsForAcct = FS.transactions.filter(t => t.account === selected);

  // Group connected accounts by owner
  const groups = [
    { label: "Joint", filter: (a) => a.owner === "joint" },
    { label: "Mira",  filter: (a) => a.owner === "mira" },
    { label: "Adam",  filter: (a) => a.owner === "adam" },
  ];

  return (
    <div className="screen">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Accounts · 6 connected · 5 manual</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Everything in one place.</h1>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn outline sm" onClick={() => window.toast?.("Connecting to Plaid…", { sub: "Pick your bank in the popup", kind: "accent", duration: 3000 })}><I.Bank /> Connect bank</button>
          <button className="btn sm" onClick={() => window.toast?.("Manual asset form", { sub: "Home, vehicle, crypto, collectible…" })}><I.Plus /> Add manual asset</button>
        </div>
      </div>

      {/* Net worth tiles */}
      <div className="stat-row" style={{ marginTop: 14, gridTemplateColumns: "repeat(4, 1fr)" }}>
        <Stat label="Assets · connected" value={<Currency value={FS.totals.liquid + FS.totals.invested} />} sub={<span className="muted" style={{fontSize:11.5}}>checking · savings · investing</span>} />
        <Stat label="Assets · manual" value={<Currency value={FS.totals.assetTotal} />} sub={<span className="muted" style={{fontSize:11.5}}>home · vehicles · crypto</span>} />
        <Stat label="Liabilities" value={<Currency value={FS.totals.liabilityTotal} />} sub={<span className="muted" style={{fontSize:11.5}}>mortgage + 3 more</span>} />
        <Stat label="Net worth" value={<Currency value={FS.totals.netWorth} />} sub={<span className="npill pos">+$3,220 · 30d</span>} accent />
      </div>

      <div style={{ display: "grid", gridTemplateColumns: "1.1fr 1.4fr", gap: 18, marginTop: 26 }}>
        {/* LEFT: account lists */}
        <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
          {/* Connected accounts */}
          <div>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
              <div className="eyebrow"><span className="dot"></span>Connected</div>
              <span className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>Synced 2m ago</span>
            </div>
            {groups.map(g => {
              const items = FS.accounts.filter(g.filter);
              if (!items.length) return null;
              const sum = items.reduce((s, a) => s + a.balance, 0);
              return (
                <div key={g.label} style={{ marginBottom: 14 }}>
                  <div style={{ display: "flex", justifyContent: "space-between", padding: "6px 4px 8px" }}>
                    <span className="muted" style={{ fontSize: 12, fontFamily: "var(--mono)", textTransform: "uppercase", letterSpacing: "0.06em" }}>{g.label}</span>
                    <span className="num tabular muted" style={{ fontSize: 12.5 }}>{FS.fmt(sum)}</span>
                  </div>
                  <div className="card flush">
                    {items.map((a, i) => (
                      <button key={a.id}
                        onClick={() => setSelected(a.id)}
                        style={{
                          display: "grid", gridTemplateColumns: "12px 1fr 80px 90px", gap: 14, alignItems: "center",
                          width: "100%", textAlign: "left",
                          padding: "14px 16px",
                          borderBottom: i === items.length - 1 ? "none" : "1px solid var(--hairline)",
                          background: selected === a.id ? "var(--surface-2)" : "transparent",
                          cursor: "pointer",
                        }}>
                        <span className="acct-dot" style={{ background: a.color, color: a.color }}></span>
                        <div>
                          <div style={{ fontSize: 14, fontWeight: 500, color: "var(--ink)" }}>{a.name}</div>
                          <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{a.bank} · ····{a.last4}</div>
                        </div>
                        <div style={{ height: 22, opacity: 0.6 }}>
                          <Sparkline values={a.sparkline} color={a.color} height={22} />
                        </div>
                        <div className="figure" style={{ fontSize: 16, textAlign: "right", color: a.balance < 0 ? "var(--negative)" : "var(--ink)" }}>
                          <Currency value={a.balance} />
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              );
            })}
          </div>

          {/* Manual assets */}
          <div>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
              <div className="eyebrow"><span className="dot"></span>Manual assets</div>
              <span className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>Update monthly</span>
            </div>
            <div className="card flush">
              {FS.assets.map((a, i) => (
                <div key={a.id} style={{ display: "grid", gridTemplateColumns: "32px 1fr auto", gap: 14, alignItems: "center", padding: "12px 16px", borderBottom: i === FS.assets.length - 1 ? "none" : "1px solid var(--hairline)" }}>
                  <div style={{ fontSize: 18, lineHeight: 1, width: 28, height: 28, display: "grid", placeItems: "center", background: "var(--surface-2)", borderRadius: 7 }}>{a.icon}</div>
                  <div>
                    <div style={{ fontSize: 14, fontWeight: 500 }}>{a.name}</div>
                    <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{a.kind} · updated {a.updated}</div>
                  </div>
                  <div style={{ textAlign: "right" }}>
                    <div className="figure" style={{ fontSize: 15 }}>${a.value.toLocaleString()}</div>
                    <div className={`num tabular ${a.delta90 > 0 ? "pos" : a.delta90 < 0 ? "neg" : "muted"}`} style={{ fontSize: 12 }}>
                      {a.delta90 === 0 ? "—" : `${a.delta90 > 0 ? "↑" : "↓"} $${Math.abs(a.delta90).toLocaleString()}/90d`}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Liabilities */}
          <div>
            <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 10 }}>
              <div className="eyebrow"><span className="dot"></span>Liabilities</div>
              <span className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>Total {FS.fmt(FS.totals.liabilityTotal)}</span>
            </div>
            <div className="card flush">
              {FS.liabilities.map((l, i) => (
                <div key={l.id} style={{ padding: "14px 16px", borderBottom: i === FS.liabilities.length - 1 ? "none" : "1px solid var(--hairline)" }}>
                  <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center" }}>
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 500 }}>{l.name}</div>
                      <div className="muted" style={{ fontSize: 12, marginTop: 2 }}>{l.kind} · {l.rate}% APR · {l.monthly ? `$${l.monthly}/mo` : l.note}</div>
                    </div>
                    <div style={{ textAlign: "right" }}>
                      <div className="figure" style={{ fontSize: 15, color: "var(--negative)" }}>−${l.balance.toLocaleString()}</div>
                      <div className="muted" style={{ fontSize: 12 }}>paid by {l.payoff}</div>
                    </div>
                  </div>
                  {l.remaining != null && (
                    <div style={{ marginTop: 10, height: 4, background: "var(--surface-2)", borderRadius: 999, overflow: "hidden" }}>
                      <div style={{ width: `${100 - (l.remaining / 360) * 100}%`, height: "100%", background: "var(--accent)", opacity: 0.6 }} />
                    </div>
                  )}
                </div>
              ))}
            </div>
          </div>
        </div>

        {/* RIGHT: detail */}
        <div>
          <div className="card" style={{ padding: 0, overflow: "hidden" }}>
            <div style={{ padding: "22px 26px 16px", borderBottom: "1px solid var(--hairline)" }}>
              <div style={{ display: "flex", alignItems: "flex-start", justifyContent: "space-between", gap: 18 }}>
                <div>
                  <div className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>{acct.bank.toUpperCase()} · {acct.type.toUpperCase()} · ····{acct.last4}</div>
                  <div className="h1" style={{ fontSize: 24, marginTop: 6 }}>{acct.name}</div>
                </div>
                <div className="figure" style={{ fontSize: 34, lineHeight: 1, color: acct.balance < 0 ? "var(--negative)" : "var(--ink)" }}>
                  <Currency value={acct.balance} decimals={2} />
                </div>
              </div>
              <div style={{ marginTop: 12, display: "flex", gap: 8, flexWrap: "wrap" }}>
                <span className="chip"><I.Sparkle width="11" height="11" /> Auto-synced</span>
                <span className={`chip ${acct.delta30 >= 0 ? "positive" : "negative"}`}><span className="dot"></span>{acct.delta30 >= 0 ? "+" : "−"}${Math.abs(acct.delta30).toLocaleString()} · 30d</span>
                {acct.type === "Credit" && <span className="chip warning"><span className="dot"></span>Statement closes in 3d</span>}
              </div>
              <div style={{ marginTop: 18, height: 80 }}>
                <Sparkline values={acct.sparkline} color={acct.color} fill height={80} width={500} />
              </div>
            </div>

            <div style={{ padding: "12px 22px", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
              <div className="eyebrow"><span className="dot"></span>Recent activity</div>
              <div style={{ display: "flex", gap: 6 }}>
                <button className="btn ghost sm" onClick={() => window.toast?.("Filter panel", { sub: "Date · category · amount · merchant" })}><I.Filter /> Filter</button>
                <button className="btn ghost sm" onClick={() => {
                  const rows = [["date","merchant","category","amount"], ...txnsForAcct.map(t => [t.date, t.merchant, t.category, t.amount])];
                  const csv = rows.map(r => r.map(c => `"${c}"`).join(",")).join("\n");
                  const blob = new Blob([csv], { type: "text/csv" });
                  const url = URL.createObjectURL(blob);
                  const a = document.createElement("a"); a.href = url; a.download = `${acct.name.replace(/\s/g, "-")}-activity.csv`; a.click(); URL.revokeObjectURL(url);
                  window.toast?.(`Exported ${txnsForAcct.length} rows`, { kind: "success" });
                }}><I.ArrowDown /> Export</button>
              </div>
            </div>

            <table className="tbl">
              <tbody>
                {txnsForAcct.length === 0 && (
                  <tr><td colSpan={4} style={{ textAlign: "center", color: "var(--ink-faint)", padding: 28 }}>No recent activity on this account.</td></tr>
                )}
                {txnsForAcct.map(t => {
                  const cat = FS.categories.find(c => c.id === t.category);
                  const mer = FS.merchants[t.merchant] || { bg: "#3F3F46", short: t.merchant.slice(0, 2) };
                  return (
                    <tr key={t.id}>
                      <td style={{ width: 64, color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 12.5 }}>{t.date}</td>
                      <td>
                        <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                          <div style={{ width: 24, height: 24, borderRadius: 6, background: mer.bg, color: "#fff", display: "grid", placeItems: "center", fontSize: 11, fontWeight: 600 }}>{mer.short}</div>
                          <div>
                            <div style={{ fontSize: 14 }}>{t.merchant}</div>
                            {t.aiTag && <div style={{ fontSize: 11.5, color: "var(--accent)", marginTop: 1 }}>✦ {t.aiTag}</div>}
                          </div>
                        </div>
                      </td>
                      <td className="muted" style={{ fontSize: 13.5 }}>
                        <span className="cswatch" style={{ background: cat?.color, marginRight: 7 }}></span>
                        {cat?.label || "—"}
                      </td>
                      <td className="right num tabular" style={{ color: t.amount > 0 ? "var(--positive)" : "var(--ink)" }}>
                        {FS.fmt(t.amount, { decimals: 2, signed: t.amount > 0 })}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}
window.Accounts = Accounts;
