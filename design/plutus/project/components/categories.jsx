function Categories() {
  const [scope, setScope] = React.useState("month"); // month | avg | year
  const cats = FS.categories.filter(c => c.thisMonth + c.lastMonth + c.budget > 0).slice();
  cats.sort((a, b) => b.thisMonth - a.thisMonth);

  // Period-aware "current" + "compare" values
  const valueFor = (c) => scope === "year" ? c.yearAvg * 12 : scope === "avg" ? c.yearAvg : c.thisMonth;
  const compareFor = (c) => scope === "year" ? c.yearAvg * 5 : scope === "avg" ? c.thisMonth : c.lastMonth;
  const valueLabel = scope === "year" ? "12-month total" : scope === "avg" ? "12-mo average" : "This month";
  const compareLabel = scope === "year" ? "YTD" : scope === "avg" ? "This month" : "April";

  const totalThis = cats.reduce((s, c) => s + valueFor(c), 0);
  const totalLast = cats.reduce((s, c) => s + compareFor(c), 0);

  const biggestDrop = cats.slice().sort((a, b) => (valueFor(a) - compareFor(a)) - (valueFor(b) - compareFor(b)))[0];
  const biggestRise = cats.slice().sort((a, b) => (valueFor(b) - compareFor(b)) - (valueFor(a) - compareFor(a)))[0];

  return (
    <div className="screen">
      <SectionHeader
        eyebrow={`Categories · ${scope === "year" ? "YTD 2026" : scope === "avg" ? "12-mo trailing" : "May 2026"}`}
        title="Where the money is going."
        action={
          <div className="toolbar">
            <button className={scope === "month" ? "on" : ""} onClick={() => setScope("month")}>This month</button>
            <button className={scope === "avg" ? "on" : ""} onClick={() => setScope("avg")}>vs. average</button>
            <button className={scope === "year" ? "on" : ""} onClick={() => setScope("year")}>Year</button>
          </div>
        }
      />

      <div className="card" style={{ marginTop: 8 }}>
        <div style={{ display: "flex", justifyContent: "space-between", alignItems: "baseline", marginBottom: 14 }}>
          <div>
            <div className="eyebrow" style={{ marginBottom: 6 }}>{valueLabel}</div>
            <div className="figure" style={{ fontSize: 44, lineHeight: 1 }}><Currency value={totalThis} /></div>
          </div>
          <div style={{ textAlign: "right" }}>
            <div className="muted" style={{ fontSize: 13.5 }}>vs. {compareLabel}</div>
            <div className={`num ${totalThis < totalLast ? "pos" : "neg"}`} style={{ fontSize: 18 }}>
              {totalThis < totalLast ? "↓" : "↑"} {FS.fmt(Math.abs(totalLast - totalThis))} · {totalLast === 0 ? "—" : Math.round(Math.abs((totalLast - totalThis) / totalLast) * 100) + "%"}
            </div>
          </div>
        </div>

        <div className="stream" style={{ height: 18, borderRadius: 6 }}>
          {cats.map(c => (
            <span key={c.id} title={`${c.label} · ${FS.fmt(valueFor(c))}`} style={{ width: `${(valueFor(c) / totalThis) * 100}%`, background: c.color }} />
          ))}
        </div>

        <p style={{ marginTop: 18, color: "var(--ink-mute)", fontSize: 14, lineHeight: 1.6, maxWidth: 720 }}>
          <I.Sparkle style={{ color: "var(--accent)", verticalAlign: "-3px", marginRight: 6 }} />
          {biggestDrop && <><span className="strong">{biggestDrop.label}</span> dropped <span className="strong">{FS.fmt(Math.abs(compareFor(biggestDrop) - valueFor(biggestDrop)))}</span> — the biggest move.</>}
          {biggestRise && <span className="muted"> {biggestRise.label} rose by {FS.fmt(Math.abs(valueFor(biggestRise) - compareFor(biggestRise)))}.</span>}
        </p>
      </div>

      <div className="section">
        <div className="card flush">
          <div className="card-head">
            <div className="h3">All categories</div>
            <div className="muted" style={{ fontSize: 13 }}>Budget set by you · agent suggests adjustments quarterly</div>
          </div>
          <table className="tbl">
            <thead>
              <tr>
                <th style={{ width: "30%" }}>Category</th>
                <th>Pace</th>
                <th className="right">{valueLabel}</th>
                <th className="right">{compareLabel}</th>
                <th className="right">Budget</th>
                <th className="right">Transactions</th>
              </tr>
            </thead>
            <tbody>
              {cats.map(c => {
                const v = valueFor(c);
                const budget = scope === "year" ? c.budget * 12 : c.budget;
                const pct = budget > 0 ? Math.min(120, (v / budget) * 100) : 0;
                const over = v > budget;
                const CatIco = I.catIcon[c.id] || I.Tag;
                return (
                  <tr key={c.id}>
                    <td>
                      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                        <span style={{ width: 22, height: 22, borderRadius: 6, background: "var(--surface-2)", color: c.color, display: "grid", placeItems: "center" }}>
                          <CatIco />
                        </span>
                        <span style={{ fontSize: 14 }}>{c.label}</span>
                      </div>
                    </td>
                    <td style={{ paddingTop: 8, paddingBottom: 8 }}>
                      <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                        <div style={{ flex: 1, height: 6, background: "var(--hairline)", borderRadius: 999, overflow: "hidden", maxWidth: 200 }}>
                          <div style={{ width: pct + "%", height: "100%", background: over ? "var(--negative)" : c.color, borderRadius: 999 }} />
                        </div>
                        <span className="num tabular" style={{ fontSize: 12.5, color: over ? "var(--negative)" : "var(--ink-faint)" }}>{Math.round(pct)}%</span>
                      </div>
                    </td>
                    <td className="right num tabular"><Currency value={v} /></td>
                    <td className="right num tabular muted"><Currency value={compareFor(c)} /></td>
                    <td className="right num tabular muted"><Currency value={budget} /></td>
                    <td className="right num tabular muted">{c.txns}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
}
window.Categories = Categories;
