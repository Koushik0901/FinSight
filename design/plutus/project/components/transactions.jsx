/* Transactions — the big table, with merchant logos, splits, trips, reimbursables */

function Transactions() {
  const [q, setQ] = React.useState("");
  const [filter, setFilter] = React.useState("all");
  const [selected, setSelected] = React.useState(null);

  const filters = [
    { id: "all",         label: "All" },
    { id: "uncat",       label: "Needs review", count: 1 },
    { id: "splits",      label: "Split", count: 1 },
    { id: "reimb",       label: "Reimbursable", count: 1 },
    { id: "anomaly",     label: "Anomalies", count: 1 },
    { id: "trips",       label: "Trips" },
  ];

  const items = FS.transactions.filter(t => {
    if (q && !t.merchant.toLowerCase().includes(q.toLowerCase()) && !(t.note || "").toLowerCase().includes(q.toLowerCase())) return false;
    if (filter === "uncat") return (t.confidence || 1) < 0.85;
    if (filter === "splits") return !!t.splits;
    if (filter === "reimb") return !!t.reimbursable;
    if (filter === "anomaly") return !!t.anomaly;
    return true;
  });

  return (
    <div className="screen">
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Transactions · May 2026 · 1,247 indexed</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>Every line of activity, searchable.</h1>
        </div>
        <div style={{ display: "flex", gap: 8 }}>
          <button className="btn outline sm" onClick={() => {
            const rows = [["date","merchant","category","account","amount"], ...items.map(t => [t.date, t.merchant, t.category, t.account, t.amount])];
            const csv = rows.map(r => r.map(c => `"${String(c).replace(/"/g, '\\"')}"`).join(",")).join("\n");
            const blob = new Blob([csv], { type: "text/csv" });
            const url = URL.createObjectURL(blob);
            const a = document.createElement("a"); a.href = url; a.download = `plutus-transactions-may26.csv`; a.click(); URL.revokeObjectURL(url);
            window.toast?.(`Exported ${items.length} transactions`, { kind: "success", sub: "CSV download started" });
          }}><I.ArrowDown /> Export</button>
          <button className="btn sm" onClick={() => window.toast?.("Manual entry opened", { sub: "Fill the form in the side panel", kind: "accent" })}><I.Plus /> Add manual</button>
        </div>
      </div>

      {/* Search */}
      <div style={{ marginTop: 14, display: "flex", gap: 10, alignItems: "center" }}>
        <div style={{ flex: 1, display: "flex", alignItems: "center", gap: 10, padding: "8px 14px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10 }}>
          <I.Search style={{ color: "var(--ink-faint)" }} />
          <input value={q} onChange={e => setQ(e.target.value)} placeholder="Search by merchant, note, amount, or category…"
            style={{ flex: 1, background: "transparent", border: 0, outline: 0, color: "var(--ink)", fontSize: 14 }} />
          {q && <span className="kbd">{items.length} results</span>}
        </div>
        <div className="toolbar">
          {filters.map(f => (
            <button key={f.id} className={filter === f.id ? "on" : ""} onClick={() => setFilter(f.id)}>
              {f.label}{f.count != null && <span style={{ marginLeft: 5, color: "var(--ink-faint)" }}>{f.count}</span>}
            </button>
          ))}
        </div>
      </div>

      {/* Trips strip */}
      {filter === "trips" && (
        <div style={{ marginTop: 18, display: "grid", gridTemplateColumns: "repeat(3, 1fr)", gap: 12 }}>
          {FS.trips.map(tr => (
            <div key={tr.id} className="card tight" style={{ padding: 16 }}>
              <div className="eyebrow" style={{ marginBottom: 6 }}>{tr.status}</div>
              <div className="h3" style={{ marginBottom: 4 }}>{tr.name}</div>
              <div className="muted" style={{ fontSize: 13 }}>{tr.from} → {tr.to}</div>
              <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginTop: 10 }}>
                <span className="figure" style={{ fontSize: 20 }}>${tr.spent.toLocaleString()}</span>
                <span className="muted" style={{ fontSize: 12.5, fontFamily: "var(--mono)" }}>{tr.txns} txns</span>
              </div>
              {tr.target && (
                <div style={{ marginTop: 10 }}>
                  <div className="muted" style={{ fontSize: 12, marginBottom: 4 }}>Budget {FS.fmt(tr.target)}</div>
                  <div className="goal-bar" style={{ height: 4 }}><span style={{ width: `${(tr.spent / tr.target) * 100}%` }}></span></div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Table + detail */}
      <div style={{ marginTop: 18, display: "grid", gridTemplateColumns: selected ? "1.5fr 1fr" : "1fr", gap: 18 }}>
        <div className="card flush">
          <table className="tbl">
            <thead>
              <tr>
                <th>Date</th>
                <th>Merchant</th>
                <th>Category</th>
                <th>Account</th>
                <th className="right">Amount</th>
              </tr>
            </thead>
            <tbody>
              {items.map(t => {
                const cat = FS.categories.find(c => c.id === t.category);
                const acc = FS.accounts.find(a => a.id === t.account);
                const mer = FS.merchants[t.merchant] || { bg: "#3F3F46", short: t.merchant.slice(0, 2) };
                const lowConf = (t.confidence || 1) < 0.85;
                return (
                  <tr key={t.id} onClick={() => setSelected(t.id === selected ? null : t.id)} style={{ cursor: "pointer", background: t.id === selected ? "var(--surface-2)" : undefined }}>
                    <td style={{ width: 76, color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 12.5 }}>{t.date}</td>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: 11 }}>
                        <div style={{ width: 26, height: 26, borderRadius: 7, background: mer.bg, color: "#fff", display: "grid", placeItems: "center", fontSize: 11.5, fontWeight: 600, letterSpacing: "-0.01em" }}>{mer.short}</div>
                        <div>
                          <div style={{ fontSize: 14, display: "flex", alignItems: "center", gap: 8 }}>
                            {t.merchant}
                            {t.splits && <span className="chip" style={{ padding: "1px 7px", fontSize: 11 }}>split {t.splits.length}</span>}
                            {t.reimbursable && <span className="chip warning" style={{ padding: "1px 7px", fontSize: 11 }}>reimbursable</span>}
                            {t.anomaly && <span className="chip negative" style={{ padding: "1px 7px", fontSize: 11 }}>{t.anomaly.note}</span>}
                            {lowConf && <span className="chip" style={{ padding: "1px 7px", fontSize: 11, color: "var(--warning)", borderColor: "var(--warning)" }}>review</span>}
                          </div>
                          {t.note && <div className="muted" style={{ fontSize: 12, marginTop: 1 }}>{t.note}</div>}
                          {t.aiTag && <div style={{ fontSize: 11.5, color: "var(--accent)", marginTop: 1 }}>✦ {t.aiTag}</div>}
                        </div>
                      </div>
                    </td>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: 7 }}>
                        <span className="cswatch" style={{ background: cat?.color || "#666" }}></span>
                        <span style={{ fontSize: 13.5, color: "var(--ink-2)" }}>{cat?.label || "—"}</span>
                      </div>
                    </td>
                    <td className="muted" style={{ fontSize: 13 }}>
                      <span className="acct-dot" style={{ background: acc?.color, boxShadow: "none" }}></span>
                      {acc?.name?.replace(" · Checking", "").replace("Joint ", "J · ")}
                    </td>
                    <td className="right num tabular" style={{ color: t.amount > 0 ? "var(--positive)" : "var(--ink)", fontSize: 14 }}>
                      {FS.fmt(t.amount, { decimals: 2, signed: t.amount > 0 })}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {selected && <TxnDetail t={FS.transactions.find(t => t.id === selected)} onClose={() => setSelected(null)} />}
      </div>
    </div>
  );
}

function TxnDetail({ t, onClose }) {
  if (!t) return null;
  const cat = FS.categories.find(c => c.id === t.category);
  const acc = FS.accounts.find(a => a.id === t.account);
  const mer = FS.merchants[t.merchant] || { bg: "#3F3F46", short: t.merchant.slice(0, 2) };

  return (
    <div className="card" style={{ padding: 22, position: "sticky", top: 12, alignSelf: "flex-start" }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: 18 }}>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <div style={{ width: 44, height: 44, borderRadius: 10, background: mer.bg, color: "#fff", display: "grid", placeItems: "center", fontSize: 16, fontWeight: 600 }}>{mer.short}</div>
          <div>
            <div className="h3" style={{ marginBottom: 2 }}>{t.merchant}</div>
            <div className="muted" style={{ fontSize: 12.5 }}>{t.date} · {acc?.name}</div>
          </div>
        </div>
        <button className="btn ghost sm" onClick={onClose}><I.X /></button>
      </div>

      <div className="figure" style={{ fontSize: 36, lineHeight: 1, color: t.amount > 0 ? "var(--positive)" : "var(--ink)", letterSpacing: "-0.03em", marginBottom: 6 }}>
        {FS.fmt(t.amount, { decimals: 2, signed: t.amount > 0 })}
      </div>
      <div className="muted" style={{ fontSize: 13 }}>Confidence {Math.round((t.confidence || 1) * 100)}% · {t.status}</div>

      {/* Category */}
      <div style={{ marginTop: 22, padding: "12px 14px", background: "var(--surface-2)", borderRadius: 10 }}>
        <div className="eyebrow" style={{ marginBottom: 8 }}>Category</div>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <span className="cswatch" style={{ background: cat?.color }}></span>
            <span style={{ fontSize: 14 }}>{cat?.label}</span>
          </div>
          <button className="btn ghost sm" onClick={() => {
            const opts = FS.categories.map(c => c.label).join("\n");
            const pick = prompt(`Pick a category for \"${t.merchant}\":\n\n${opts}\n\nCurrent: ${cat?.label}`, cat?.label || "");
            if (pick) {
              const match = FS.categories.find(c => c.label.toLowerCase() === pick.toLowerCase());
              if (match) window.toast?.(`Re-categorized to ${match.label}`, { kind: "success", sub: `\"${t.merchant}\" · agent learned this`, action: { label: "Undo", onClick: () => window.toast?.("Restored original") } });
              else window.toast?.(`No category matches \"${pick}\"`, { kind: "error" });
            }
          }}>Change</button>
        </div>
      </div>

      {/* Splits */}
      {t.splits && (
        <div style={{ marginTop: 14 }}>
          <div className="eyebrow" style={{ marginBottom: 10 }}>Split into {t.splits.length}</div>
          {t.splits.map((s, i) => {
            const c = FS.categories.find(x => x.id === s.category);
            return (
              <div key={i} style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "8px 0", borderBottom: i < t.splits.length - 1 ? "1px solid var(--hairline)" : "0" }}>
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span className="cswatch" style={{ background: c?.color }}></span>
                  <span style={{ fontSize: 13.5 }}>{c?.label}</span>
                </div>
                <span className="num tabular" style={{ fontSize: 14 }}>${Math.abs(s.amount).toFixed(2)}</span>
              </div>
            );
          })}
          <button className="btn outline sm" style={{ marginTop: 12, width: "100%", justifyContent: "center" }} onClick={() => window.toast?.("Split row added", { sub: "Allocate the remaining amount across categories" })}><I.Plus /> Add split</button>
        </div>
      )}

      {/* Reimbursable */}
      {t.reimbursable && (
        <div style={{ marginTop: 14, padding: 14, background: "var(--warning-2)", borderRadius: 10, border: "1px solid var(--warning)" }}>
          <div className="eyebrow" style={{ marginBottom: 6, color: "var(--warning)" }}>Reimbursable · {t.reimbursable.state}</div>
          <div style={{ fontSize: 13.5 }}>From <span className="strong">{t.reimbursable.from}</span></div>
          <button className="btn sm" style={{ marginTop: 10 }} onClick={() => window.toast?.("Marked received", { kind: "success", sub: `$${Math.abs(t.amount).toFixed(2)} reimbursement cleared` })}>Mark received</button>
        </div>
      )}

      {/* Anomaly */}
      {t.anomaly && (
        <div style={{ marginTop: 14, padding: 14, background: "var(--negative-2)", borderRadius: 10 }}>
          <div className="eyebrow" style={{ marginBottom: 6, color: "var(--negative)" }}>Anomaly detected</div>
          <div style={{ fontSize: 13.5 }}>{t.anomaly.note}. Want to investigate the cause?</div>
        </div>
      )}

      {/* Note */}
      <div style={{ marginTop: 14 }}>
        <div className="eyebrow" style={{ marginBottom: 8 }}>Note</div>
        <textarea defaultValue={t.note || ""} placeholder="Add context for future you…"
          style={{ width: "100%", minHeight: 60, padding: 10, background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 8, color: "var(--ink)", fontSize: 14, resize: "vertical" }} />
      </div>

      {/* Attachments */}
      <div style={{ marginTop: 14 }}>
        <div className="eyebrow" style={{ marginBottom: 8 }}>Attachments</div>
        {t.attachments ? (
          <div style={{ display: "flex", gap: 8 }}>
            <div style={{ width: 56, height: 72, background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 6, display: "grid", placeItems: "center", fontSize: 11, color: "var(--ink-mute)", fontFamily: "var(--mono)" }}>RECEIPT.pdf</div>
            <button className="btn outline sm" style={{ alignSelf: "center" }} onClick={() => window.toast?.("Receipt picker opened")}><I.Plus /> Add</button>
          </div>
        ) : (
          <button className="btn outline sm" onClick={() => window.toast?.("Receipt picker opened", { sub: "Drop a PDF or photo here" })}><I.Plus /> Attach receipt</button>
        )}
      </div>

      {/* Quick actions */}
      <div style={{ marginTop: 18, display: "flex", gap: 6, flexWrap: "wrap" }}>
        <button className="btn ghost sm" onClick={() => window.toast?.("Pick a trip", { sub: "Lisbon, Tahoe, or Italy (planned)" })}>Tag as trip</button>
        <button className="btn ghost sm" onClick={() => window.toast?.("Split mode active", { sub: "Allocate this charge across categories" })}>Split</button>
        <button className="btn ghost sm" onClick={() => window.toast?.("Marked reimbursable", { kind: "warn", sub: "From whom?" })}>Reimbursable</button>
        <button className="btn ghost sm" onClick={() => window.toast?.("Marked as transfer", { sub: "Excluded from spend totals" })}>Mark as transfer</button>
      </div>
    </div>
  );
}

window.Transactions = Transactions;
