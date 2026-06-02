/* Reports — config drawer (per-widget settings), widget library modal, empty state */

/* ────────────────────────────────────────────────────────
   WidgetConfig — slide-in panel on the right when a widget
   is selected. Shape adapts to the widget's type.
   ──────────────────────────────────────────────────────── */
function WidgetConfig({ widget, onChange, onClose, onRemove }) {
  const t = WIDGET_TYPES[widget.type];
  const cfg = widget.config || {};
  const set = (patch) => onChange({ config: patch });
  const setTop = (patch) => onChange(patch);

  // Esc to close
  React.useEffect(() => {
    const k = (e) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);

  return (
    <div className="cfg-drawer" onClick={(e) => e.stopPropagation()}>
      <div className="cfg-head">
        <div>
          <div className="eyebrow"><span className="dot"></span>Configure widget</div>
          <div className="h3" style={{ marginTop: 4 }}>{t?.name || widget.type}</div>
        </div>
        <button className="close" onClick={onClose} title="Close"><I.X width="14" height="14" /></button>
      </div>

      <div className="cfg-body">
        {/* ── Basics — title + size — apply to every widget ── */}
        <div className="cfg-section">Basics</div>
        <div className="cfg-field">
          <label className="label">Title</label>
          <input
            className="cfg-input"
            value={widget.title}
            onChange={(e) => setTop({ title: e.target.value })}
          />
        </div>
        <div className="cfg-field">
          <label className="label">Subtitle <span className="muted" style={{ fontWeight: 400, fontSize: 11.5 }}>· optional</span></label>
          <input
            className="cfg-input"
            placeholder="e.g. May 2026 · year to date"
            value={cfg.subtitle || ""}
            onChange={(e) => set({ subtitle: e.target.value })}
          />
        </div>
        <div className="cfg-field">
          <label className="label">Width</label>
          <div className="cfg-size-row">
            {[1, 2, 3, 4].map(s => (
              <button key={s} className={widget.w === s ? "on" : ""} onClick={() => setTop({ w: s })}>
                {s}/4
              </button>
            ))}
          </div>
        </div>

        {/* ── Per-type config ── */}
        <KPIConfig widget={widget} set={set} />
        <SparkKPIConfig widget={widget} set={set} />
        <YoYConfig widget={widget} set={set} />
        <DonutConfig widget={widget} set={set} />
        <TableConfig widget={widget} set={set} />
        <NoteConfig widget={widget} set={set} />
        <NetWorthConfig widget={widget} set={set} />
        <BarsConfig widget={widget} set={set} />
        <SankeyConfig widget={widget} set={set} />
      </div>

      <div className="cfg-foot">
        <button className="btn ghost sm" style={{ color: "var(--negative)" }} onClick={() => {
          if (confirm("Remove this widget?")) onRemove();
        }}>
          <I.Trash width="13" height="13" /> Remove
        </button>
        <button className="btn primary sm" onClick={onClose}>
          <I.Check width="13" height="13" /> Done
        </button>
      </div>
    </div>
  );
}

/* ── Per-widget config sections ─────────────────────── */

function KPIConfig({ widget, set }) {
  if (widget.type !== "kpi") return null;
  const cfg = widget.config || {};
  const groups = {};
  METRIC_OPTIONS.forEach(m => { (groups[m.group] = groups[m.group] || []).push(m); });
  return (
    <>
      <div className="cfg-section">Data</div>
      {Object.entries(groups).map(([g, items]) => (
        <div className="cfg-field" key={g}>
          <label className="label">{g}</label>
          <div className="cfg-chips">
            {items.map(opt => (
              <button
                key={opt.id}
                className={cfg.metric === opt.id ? "on" : ""}
                onClick={() => set({ metric: opt.id })}
              >{opt.label}</button>
            ))}
          </div>
        </div>
      ))}
      <div className="cfg-section">Display</div>
      <div className="cfg-toggle">
        <span className="label">Use accent color</span>
        <span className={`tog ${cfg.accent ? "on" : ""}`} onClick={() => set({ accent: !cfg.accent })}></span>
      </div>
      <div className="cfg-field">
        <label className="label">Compare with</label>
        <div className="cfg-segments">
          {[
            { id: "none",     label: "Nothing" },
            { id: "lastMonth",label: "Last month" },
            { id: "lastYear", label: "Last year" },
            { id: "ytd",      label: "YTD" },
          ].map(o => (
            <button
              key={o.id}
              className={cfg.compare === o.id ? "on" : ""}
              onClick={() => set({ compare: o.id })}
            >{o.label}</button>
          ))}
        </div>
      </div>
    </>
  );
}

function SparkKPIConfig({ widget, set }) {
  if (widget.type !== "sparkkpi") return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Data</div>
      <div className="cfg-field">
        <label className="label">Metric</label>
        <select className="cfg-select" value={cfg.metric || "expense"} onChange={(e) => set({ metric: e.target.value })}>
          {METRIC_OPTIONS.map(o => <option key={o.id} value={o.id}>{o.label}</option>)}
        </select>
      </div>
      <div className="cfg-field">
        <label className="label">Time range</label>
        <div className="cfg-segments">
          {["3M", "6M", "12M", "YTD", "All"].map(r => (
            <button key={r} className={cfg.range === r ? "on" : ""} onClick={() => set({ range: r })}>{r}</button>
          ))}
        </div>
      </div>
    </>
  );
}

function YoYConfig({ widget, set }) {
  if (!["yoy", "cumulative"].includes(widget.type)) return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Data</div>
      <div className="cfg-field">
        <label className="label">Metric</label>
        <div className="cfg-segments">
          {[
            { id: "expense", label: "Spending" },
            { id: "income",  label: "Income" },
            { id: "net",     label: "Net flow" },
          ].map(o => (
            <button key={o.id} className={cfg.metric === o.id ? "on" : ""} onClick={() => set({ metric: o.id })}>
              {o.label}
            </button>
          ))}
        </div>
      </div>
      <div className="cfg-field">
        <label className="label">Compare</label>
        <div className="cfg-segments">
          {[
            { id: "lastYear", label: "Last year" },
            { id: "average",  label: "12-mo avg" },
            { id: "budget",   label: "Budget" },
          ].map(o => (
            <button key={o.id} className={(cfg.compare || "lastYear") === o.id ? "on" : ""} onClick={() => set({ compare: o.id })}>
              {o.label}
            </button>
          ))}
        </div>
      </div>
    </>
  );
}

function DonutConfig({ widget, set }) {
  if (widget.type !== "donut") return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Data</div>
      <div className="cfg-field">
        <label className="label">Group by</label>
        <div className="cfg-segments">
          {[
            { id: "category", label: "Category" },
            { id: "merchant", label: "Merchant" },
            { id: "account",  label: "Account" },
          ].map(o => (
            <button key={o.id} className={(cfg.dimension || "category") === o.id ? "on" : ""} onClick={() => set({ dimension: o.id })}>
              {o.label}
            </button>
          ))}
        </div>
      </div>
      <div className="cfg-field">
        <label className="label">Time range</label>
        <div className="cfg-segments">
          {["1M", "3M", "6M", "YTD", "12M"].map(r => (
            <button key={r} className={(cfg.range || "1M") === r ? "on" : ""} onClick={() => set({ range: r })}>{r}</button>
          ))}
        </div>
      </div>
      <div className="cfg-field">
        <label className="label">Filter categories <span className="muted" style={{ fontWeight: 400, fontSize: 11.5 }}>· optional</span></label>
        <div className="cfg-chips">
          {FS.categories.map(c => {
            const excluded = (cfg.exclude || []).includes(c.id);
            return (
              <button
                key={c.id}
                className={!excluded ? "on" : ""}
                onClick={() => {
                  const cur = new Set(cfg.exclude || []);
                  if (cur.has(c.id)) cur.delete(c.id); else cur.add(c.id);
                  set({ exclude: [...cur] });
                }}
              >
                <span className="cswatch" style={{ background: c.color, width: 8, height: 8 }}></span>
                {c.label}
              </button>
            );
          })}
        </div>
      </div>
    </>
  );
}

function TableConfig({ widget, set }) {
  if (!["tableCat", "tableMer", "trends"].includes(widget.type)) return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Display</div>
      <div className="cfg-field">
        <label className="label">Sort by</label>
        <div className="cfg-segments">
          {widget.type === "tableMer" ? (
            <>
              <button className={(cfg.sort || "total") === "total" ? "on" : ""} onClick={() => set({ sort: "total" })}>By total</button>
              <button className={cfg.sort === "count" ? "on" : ""} onClick={() => set({ sort: "count" })}>By count</button>
            </>
          ) : (
            <>
              <button className={(cfg.sort || "yearTotal") === "yearTotal" ? "on" : ""} onClick={() => set({ sort: "yearTotal" })}>12-mo total</button>
              <button className={cfg.sort === "thisMonth" ? "on" : ""} onClick={() => set({ sort: "thisMonth" })}>This month</button>
              <button className={cfg.sort === "delta" ? "on" : ""} onClick={() => set({ sort: "delta" })}>Largest change</button>
            </>
          )}
        </div>
      </div>
      <div className="cfg-field">
        <label className="label">Show</label>
        <div className="cfg-segments">
          {[5, 8, 10, 15, 20].map(n => (
            <button key={n} className={(cfg.limit || (widget.type === "tableMer" ? 10 : 8)) === n ? "on" : ""} onClick={() => set({ limit: n })}>
              {n}
            </button>
          ))}
        </div>
      </div>
    </>
  );
}

function NoteConfig({ widget, set }) {
  if (widget.type !== "note") return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Content</div>
      <div className="cfg-field">
        <label className="label">Body</label>
        <textarea
          className="cfg-textarea"
          rows={8}
          placeholder="Write a note, link a goal, leave a thought…"
          value={cfg.body || ""}
          onChange={(e) => set({ body: e.target.value })}
        />
        <div className="desc">Plain text. Use blank lines for paragraphs.</div>
      </div>
    </>
  );
}

function NetWorthConfig({ widget, set }) {
  if (widget.type !== "networth") return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Time</div>
      <div className="cfg-field">
        <label className="label">Range</label>
        <div className="cfg-segments">
          {["6M", "12M", "3Y", "5Y", "All"].map(r => (
            <button key={r} className={(cfg.range || "12M") === r ? "on" : ""} onClick={() => set({ range: r })}>{r}</button>
          ))}
        </div>
      </div>
      <div className="cfg-section">Display</div>
      <div className="cfg-toggle">
        <span className="label">Stack assets vs liabilities</span>
        <span className={`tog ${cfg.stack !== false ? "on" : ""}`} onClick={() => set({ stack: !(cfg.stack !== false) })}></span>
      </div>
      <div className="cfg-toggle">
        <span className="label">Show net-worth line</span>
        <span className={`tog ${cfg.showNet !== false ? "on" : ""}`} onClick={() => set({ showNet: !(cfg.showNet !== false) })}></span>
      </div>
    </>
  );
}

function BarsConfig({ widget, set }) {
  if (widget.type !== "bars") return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Time</div>
      <div className="cfg-field">
        <label className="label">Range</label>
        <div className="cfg-segments">
          {["3M", "6M", "12M", "YTD", "All"].map(r => (
            <button key={r} className={(cfg.range || "6M") === r ? "on" : ""} onClick={() => set({ range: r })}>{r}</button>
          ))}
        </div>
      </div>
    </>
  );
}

function SankeyConfig({ widget, set }) {
  if (widget.type !== "sankey") return null;
  const cfg = widget.config || {};
  return (
    <>
      <div className="cfg-section">Data</div>
      <div className="cfg-field">
        <label className="label">Period</label>
        <div className="cfg-segments">
          {[
            { id: "current", label: "This month" },
            { id: "ytd",     label: "Year to date" },
            { id: "12M",     label: "Last 12 mo" },
          ].map(o => (
            <button key={o.id} className={(cfg.month || "current") === o.id ? "on" : ""} onClick={() => set({ month: o.id })}>
              {o.label}
            </button>
          ))}
        </div>
      </div>
      <div className="cfg-toggle">
        <span className="label">Highlight savings flow</span>
        <span className={`tog ${cfg.highlightSave !== false ? "on" : ""}`} onClick={() => set({ highlightSave: !(cfg.highlightSave !== false) })}></span>
      </div>
    </>
  );
}

/* ────────────────────────────────────────────────────────
   WidgetLibrary — modal to pick a new widget from a catalog
   ──────────────────────────────────────────────────────── */
function WidgetLibrary({ onPick, onClose }) {
  const [query, setQuery] = React.useState("");
  React.useEffect(() => {
    const k = (e) => { if (e.key === "Escape") onClose(); };
    window.addEventListener("keydown", k);
    return () => window.removeEventListener("keydown", k);
  }, [onClose]);

  const grouped = {};
  Object.entries(WIDGET_TYPES).forEach(([id, t]) => {
    if (query) {
      const q = query.toLowerCase();
      if (!t.name.toLowerCase().includes(q) && !t.desc.toLowerCase().includes(q)) return;
    }
    (grouped[t.group] = grouped[t.group] || []).push({ id, ...t });
  });

  return (
    <div className="lib-mask" onClick={onClose}>
      <div className="lib" onClick={(e) => e.stopPropagation()}>
        <div className="lib-head">
          <div>
            <div className="eyebrow"><span className="dot"></span>Widget library</div>
            <div className="h1" style={{ marginTop: 4 }}>What do you want to see?</div>
          </div>
          <button className="close" onClick={onClose}><I.X width="16" height="16" /></button>
        </div>

        <div style={{ padding: "0 24px" }}>
          <div className="cmdk-input" style={{ padding: "12px 0", borderBottom: "1px solid var(--hairline)" }}>
            <I.Search width="14" height="14" style={{ color: "var(--ink-mute)" }} />
            <input
              autoFocus
              placeholder="Search widgets — net worth, dining, donut…"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              style={{ flex: 1, background: "transparent", border: 0, outline: 0, color: "var(--ink)", fontSize: 14 }}
            />
            <span style={{ fontSize: 11, color: "var(--ink-faint)", fontFamily: "var(--mono)" }}>esc to close</span>
          </div>
        </div>

        <div className="lib-body">
          {WIDGET_GROUP_ORDER.map(g => grouped[g] && (
            <div key={g} className="lib-group">
              <div className="group-label">{g}</div>
              <div className="lib-grid">
                {grouped[g].map(w => (
                  <button key={w.id} className="lib-card" onClick={() => onPick(w.id)}>
                    <div className="row">
                      <div className="ic"><WidgetIcon kind={w.iconKind} /></div>
                      <div className="name">{w.name}</div>
                    </div>
                    <WidgetPreview kind={w.iconKind} />
                    <div className="desc">{w.desc}</div>
                    <div className="meta">
                      <span>{w.group}</span>
                      <span className="size">{w.defaultW}×{w.defaultH}</span>
                    </div>
                  </button>
                ))}
              </div>
            </div>
          ))}
          {Object.keys(grouped).length === 0 && (
            <div style={{ padding: 60, textAlign: "center", color: "var(--ink-mute)" }}>
              No widgets match "<span className="strong">{query}</span>" yet.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function WidgetIcon({ kind }) {
  switch (kind) {
    case "kpi":      return <I.Sparkle width="14" height="14" />;
    case "sparkkpi": return <I.Activity width="14" height="14" />;
    case "line":     return <I.Line width="14" height="14" />;
    case "bar":      return <I.Bar width="14" height="14" />;
    case "donut":    return <I.Donut width="14" height="14" />;
    case "table":    return <I.ListI width="14" height="14" />;
    case "flow":     return <I.Flow width="14" height="14" />;
    case "grid":     return <I.Grid width="14" height="14" />;
    case "text":     return <I.TextIco width="14" height="14" />;
    default:         return <I.Grid width="14" height="14" />;
  }
}

function WidgetPreview({ kind }) {
  // tiny chart-shape preview to make the catalog scannable
  if (kind === "kpi") {
    return (
      <div className="prev" style={{ alignItems: "center", justifyContent: "flex-start" }}>
        <span style={{ fontFamily: "var(--sans)", fontSize: 18, fontWeight: 600, color: "var(--ink)", letterSpacing: "-0.03em" }}>37<span style={{ color: "var(--ink-mute)", fontSize: 12 }}>%</span></span>
      </div>
    );
  }
  if (kind === "sparkkpi") {
    return (
      <div className="prev" style={{ alignItems: "center", padding: 6 }}>
        <span style={{ fontFamily: "var(--sans)", fontSize: 13, fontWeight: 600, color: "var(--ink)", marginRight: 6, letterSpacing: "-0.02em" }}>$3.5k</span>
        <svg width="60" height="22" viewBox="0 0 60 22" style={{ flex: 1 }}>
          <path d="M0 18 L10 14 L18 16 L28 10 L38 12 L48 6 L60 4" fill="none" stroke="var(--accent)" strokeWidth="1.4" />
        </svg>
      </div>
    );
  }
  if (kind === "line") {
    return (
      <div className="prev" style={{ alignItems: "center" }}>
        <svg width="100%" height="24" viewBox="0 0 100 24">
          <path d="M0 18 L15 12 L30 16 L45 8 L60 14 L75 5 L100 9" fill="none" stroke="var(--accent)" strokeWidth="1.4" />
          <path d="M0 20 L20 18 L40 19 L60 15 L80 17 L100 14" fill="none" stroke="var(--ink-faint)" strokeWidth="1.2" strokeDasharray="2 2" opacity="0.7" />
        </svg>
      </div>
    );
  }
  if (kind === "bar") {
    return (
      <div className="prev">
        <span className="b" style={{ height: "50%" }}></span>
        <span className="b" style={{ height: "30%" }}></span>
        <span className="b" style={{ height: "70%" }}></span>
        <span className="b" style={{ height: "55%" }}></span>
        <span className="b" style={{ height: "85%" }}></span>
        <span className="b" style={{ height: "40%" }}></span>
      </div>
    );
  }
  if (kind === "donut") {
    return (
      <div className="prev" style={{ alignItems: "center", justifyContent: "center" }}>
        <svg width="28" height="28" viewBox="0 0 28 28">
          <circle cx="14" cy="14" r="11" fill="none" stroke="var(--ink-faint)" strokeWidth="5" opacity="0.4" />
          <circle cx="14" cy="14" r="11" fill="none" stroke="var(--accent)" strokeWidth="5"
            strokeDasharray={`${Math.PI * 22 * 0.62} ${Math.PI * 22}`} transform="rotate(-90 14 14)" />
        </svg>
      </div>
    );
  }
  if (kind === "table") {
    return (
      <div className="prev" style={{ flexDirection: "column", gap: 4, padding: 6 }}>
        <div style={{ display: "flex", gap: 4 }}>
          <span style={{ flex: 1, height: 4, background: "var(--ink-faint)", opacity: 0.4, borderRadius: 1 }}></span>
          <span style={{ width: 30, height: 4, background: "var(--ink-faint)", opacity: 0.4, borderRadius: 1 }}></span>
        </div>
        <div style={{ display: "flex", gap: 4 }}>
          <span style={{ flex: 1, height: 4, background: "var(--ink-mute)", opacity: 0.5, borderRadius: 1 }}></span>
          <span style={{ width: 22, height: 4, background: "var(--accent)", borderRadius: 1 }}></span>
        </div>
        <div style={{ display: "flex", gap: 4 }}>
          <span style={{ flex: 1, height: 4, background: "var(--ink-mute)", opacity: 0.5, borderRadius: 1 }}></span>
          <span style={{ width: 18, height: 4, background: "var(--accent)", borderRadius: 1, opacity: 0.6 }}></span>
        </div>
      </div>
    );
  }
  if (kind === "flow") {
    return (
      <div className="prev" style={{ alignItems: "center", justifyContent: "center" }}>
        <svg width="100%" height="22" viewBox="0 0 80 22">
          <path d="M0 5 C20 5 20 11 40 11 C60 11 60 5 80 5" fill="none" stroke="var(--accent)" strokeWidth="3" opacity="0.6" />
          <path d="M0 16 C20 16 20 11 40 11 C60 11 60 16 80 16" fill="none" stroke="var(--ink-faint)" strokeWidth="3" opacity="0.5" />
        </svg>
      </div>
    );
  }
  if (kind === "grid") {
    return (
      <div className="prev" style={{ padding: 4 }}>
        {Array.from({ length: 8 }).map((_, i) => (
          <span key={i} className="b" style={{ height: `${30 + (i * 53) % 50}%` }}></span>
        ))}
      </div>
    );
  }
  if (kind === "text") {
    return (
      <div className="prev" style={{ flexDirection: "column", gap: 3, padding: 6, justifyContent: "center" }}>
        <span style={{ width: "80%", height: 3, background: "var(--ink-faint)", opacity: 0.5, borderRadius: 1 }}></span>
        <span style={{ width: "95%", height: 3, background: "var(--ink-faint)", opacity: 0.5, borderRadius: 1 }}></span>
        <span style={{ width: "60%", height: 3, background: "var(--ink-faint)", opacity: 0.5, borderRadius: 1 }}></span>
      </div>
    );
  }
  return <div className="prev"></div>;
}

/* ────────────────────────────────────────────────────────
   EmptyDashboard — when a new report has no widgets yet
   ──────────────────────────────────────────────────────── */
function EmptyDashboard({ onAdd }) {
  return (
    <div className="empty-dash">
      <div style={{ display: "inline-flex", alignItems: "center", gap: 8, padding: "4px 10px", background: "var(--accent-2)", borderRadius: 999, marginBottom: 16 }}>
        <I.Sparkle width="12" height="12" style={{ color: "var(--accent)" }} />
        <span style={{ fontSize: 11, color: "var(--accent)", fontWeight: 500, letterSpacing: "0.06em", textTransform: "uppercase", fontFamily: "var(--mono)" }}>Empty report</span>
      </div>
      <div className="h1">Nothing here yet.</div>
      <div className="muted" style={{ fontSize: 14, maxWidth: 500, margin: "10px auto 22px", lineHeight: 1.55 }}>
        Pick a widget from the library — numbers, breakdowns, flows, notes. Drag to reorder, click to configure. Lives here, saves to this device.
      </div>
      <div style={{ display: "flex", justifyContent: "center", gap: 8 }}>
        <button className="btn primary" onClick={onAdd}>
          <I.Plus width="14" height="14" /> Add first widget
        </button>
      </div>
    </div>
  );
}

Object.assign(window, { WidgetConfig, WidgetLibrary, EmptyDashboard });
