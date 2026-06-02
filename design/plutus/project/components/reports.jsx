/* Reports — customizable widget dashboard
   Main component: state, persistence, tabs, grid, widget shell.
   Widget renderers live in reports-widgets.jsx; configurator + library in reports-config.jsx. */

const REPORTS_VERSION = 3;
const REPORTS_STORAGE_KEY = "plutus.reports.v3";

function loadReports() {
  try {
    const raw = localStorage.getItem(REPORTS_STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      if (parsed && parsed.version === REPORTS_VERSION && Array.isArray(parsed.reports)) {
        return parsed;
      }
    }
  } catch {}
  return { version: REPORTS_VERSION, reports: structuredClone(window.DEFAULT_REPORTS) };
}

function saveReports(s) {
  try { localStorage.setItem(REPORTS_STORAGE_KEY, JSON.stringify(s)); } catch {}
}

function genId(prefix) { return prefix + "_" + Math.random().toString(36).slice(2, 8); }

const SCOPE_LABELS = {
  month:   { short: "May",       long: "May 2026",       sub: "current month" },
  quarter: { short: "Quarter",   long: "Q2 (Mar–May)",   sub: "3-month rolling" },
  year:    { short: "Year",      long: "YTD 2026",       sub: "Jan → today" },
  all:     { short: "All-time",  long: "All-time",       sub: "every recorded month" },
};

function Reports() {
  const [state, setState] = React.useState(() => loadReports());
  const [editing, setEditing] = React.useState(false);
  const [activeId, setActiveId] = React.useState(() => state.reports[0]?.id);
  const [configWidgetId, setConfigWidgetId] = React.useState(null);
  const [libraryOpen, setLibraryOpen] = React.useState(false);
  const [renamingTab, setRenamingTab] = React.useState(null);
  const [dragOverId, setDragOverId] = React.useState(null);
  const [scope, setScope] = React.useState("month");
  const dragSrcId = React.useRef(null);

  // persist on every state change
  React.useEffect(() => { saveReports(state); }, [state]);

  // keep activeId valid
  React.useEffect(() => {
    if (!state.reports.find(r => r.id === activeId)) {
      setActiveId(state.reports[0]?.id);
    }
  }, [state.reports, activeId]);

  // body-level data attr for grid overlay styling
  React.useEffect(() => {
    document.documentElement.setAttribute("data-editing", editing ? "on" : "off");
    return () => document.documentElement.removeAttribute("data-editing");
  }, [editing]);

  const active = state.reports.find(r => r.id === activeId);
  if (!active) return null;

  // ── Mutators ────────────────────────────────────────
  const updateReport = (id, fn) => setState(s => ({
    ...s,
    reports: s.reports.map(r => r.id === id ? fn(r) : r)
  }));

  const updateWidget = (id, patch) => updateReport(active.id, r => ({
    ...r,
    widgets: r.widgets.map(w => w.id === id
      ? { ...w, ...patch, config: { ...(w.config || {}), ...(patch.config || {}) } }
      : w)
  }));

  const addWidget = (type) => {
    const t = WIDGET_TYPES[type];
    const id = genId("w");
    const widget = {
      id, type,
      w: t.defaultW, h: t.defaultH,
      title: t.name,
      config: structuredClone(t.defaultConfig || {})
    };
    updateReport(active.id, r => ({ ...r, widgets: [...r.widgets, widget] }));
    setLibraryOpen(false);
    setConfigWidgetId(id);
    window.toast?.(`Added \u201c${t.name}\u201d`, { sub: `Configure on the right`, kind: "success" });
  };

  const removeWidget = (id) => {
    const w = active.widgets.find(x => x.id === id);
    updateReport(active.id, r => ({ ...r, widgets: r.widgets.filter(w => w.id !== id) }));
    if (configWidgetId === id) setConfigWidgetId(null);
    if (w) window.toast?.(`Removed \u201c${w.title}\u201d`, { kind: "warn" });
  };

  const cycleSize = (id) => {
    const w = active.widgets.find(x => x.id === id);
    if (!w) return;
    const sizes = [1, 2, 3, 4];
    const next = sizes[(sizes.indexOf(w.w) + 1) % sizes.length];
    updateWidget(id, { w: next });
  };

  // ── Drag & drop reorder ─────────────────────────────
  const onDragStart = (id) => (e) => {
    dragSrcId.current = id;
    e.dataTransfer.effectAllowed = "move";
    try { e.dataTransfer.setData("text/plain", id); } catch {}
    setTimeout(() => {
      const el = document.querySelector(`[data-widget-id="${id}"]`);
      if (el) el.classList.add("dragging");
    }, 0);
  };
  const onDragOver = (id) => (e) => {
    e.preventDefault();
    e.dataTransfer.dropEffect = "move";
    if (dragSrcId.current && dragSrcId.current !== id) setDragOverId(id);
  };
  const onDragLeave = () => setDragOverId(null);
  const onDragEnd = () => {
    document.querySelectorAll(".widget.dragging").forEach(el => el.classList.remove("dragging"));
    dragSrcId.current = null;
    setDragOverId(null);
  };
  const onDrop = (targetId) => (e) => {
    e.preventDefault();
    const src = dragSrcId.current;
    dragSrcId.current = null;
    setDragOverId(null);
    if (!src || src === targetId) return;
    updateReport(active.id, r => {
      const list = [...r.widgets];
      const si = list.findIndex(w => w.id === src);
      const ti = list.findIndex(w => w.id === targetId);
      if (si < 0 || ti < 0) return r;
      const [moved] = list.splice(si, 1);
      list.splice(ti, 0, moved);
      return { ...r, widgets: list };
    });
  };

  // ── Report (tab) actions ───────────────────────────
  const newReport = () => {
    const id = genId("r");
    const r = { id, name: "Untitled report", icon: "◇", widgets: [] };
    setState(s => ({ ...s, reports: [...s.reports, r] }));
    setActiveId(id);
    setEditing(true);
    setRenamingTab(id);
  };

  const duplicateReport = (id) => {
    const r = state.reports.find(x => x.id === id);
    if (!r) return;
    const nid = genId("r");
    const copy = {
      ...r,
      id: nid,
      name: r.name + " · copy",
      widgets: r.widgets.map(w => ({ ...w, id: genId("w") }))
    };
    setState(s => ({ ...s, reports: [...s.reports, copy] }));
    setActiveId(nid);
  };

  const deleteReport = (id) => {
    if (state.reports.length <= 1) return;
    const remaining = state.reports.filter(r => r.id !== id);
    setState(s => ({ ...s, reports: remaining }));
    if (activeId === id) setActiveId(remaining[0].id);
  };

  const renameReport = (id, name) => updateReport(id, r => ({ ...r, name }));

  const resetAll = () => {
    if (!confirm("Reset every report and widget to defaults? Your customizations will be lost.")) return;
    const fresh = { version: REPORTS_VERSION, reports: structuredClone(window.DEFAULT_REPORTS) };
    setState(fresh);
    setActiveId(fresh.reports[0].id);
    setConfigWidgetId(null);
  };

  const configWidget = active.widgets.find(w => w.id === configWidgetId);
  const scopeLabel = SCOPE_LABELS[scope];

  const exportReport = () => {
    const payload = {
      exported: new Date().toISOString(),
      scope,
      report: active,
    };
    const blob = new Blob([JSON.stringify(payload, null, 2)], { type: "application/json" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `plutus-${active.name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-${scope}.json`;
    document.body.appendChild(a); a.click();
    setTimeout(() => { document.body.removeChild(a); URL.revokeObjectURL(url); }, 100);
    window.toast?.(`Exported \u201c${active.name}\u201d`, { sub: `${active.widgets.length} widgets · ${scope}`, kind: "success" });
  };

  return (
    <div className="screen">
      {/* Header */}
      <div className="day-hdr">
        <div>
          <div className="eyebrow"><span className="dot"></span>Reports · {scopeLabel.long} · build your own view</div>
          <h1 className="h1" style={{ fontSize: 28, marginTop: 6 }}>How money is moving.</h1>
        </div>
        <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
          {!editing && (
            <div className="toolbar" style={{ marginRight: 4 }}>
              {Object.entries(SCOPE_LABELS).map(([k, v]) => (
                <button key={k} className={scope === k ? "on" : ""} onClick={() => setScope(k)} title={v.sub}>
                  {v.short}
                </button>
              ))}
            </div>
          )}
          {editing ? (
            <>
              <button className="btn outline sm" onClick={() => setLibraryOpen(true)}>
                <I.Plus width="14" height="14" /> Add widget
              </button>
              <button className="btn primary sm" onClick={() => { setEditing(false); setConfigWidgetId(null); }}>
                <I.Check width="14" height="14" /> Done
              </button>
            </>
          ) : (
            <>
              <button className="btn ghost sm" title="Download as JSON" onClick={exportReport}>
                <I.ArrowDown width="14" height="14" /> Export
              </button>
              <button className="btn outline sm" onClick={() => setEditing(true)}>
                <I.Pencil width="14" height="14" /> Customize
              </button>
            </>
          )}
        </div>
      </div>

      {/* Saved reports as tabs */}
      <ReportTabs
        reports={state.reports}
        activeId={activeId}
        setActiveId={setActiveId}
        editing={editing}
        renamingId={renamingTab}
        setRenamingId={setRenamingTab}
        onRename={renameReport}
        onNew={newReport}
        onDuplicate={duplicateReport}
        onDelete={deleteReport}
      />

      {/* Widget grid */}
      <div className="wgrid" onDragLeave={onDragLeave}>
        {active.widgets.map(w => (
          <WidgetShell
            key={w.id}
            widget={w}
            scope={scope}
            editing={editing}
            configuring={w.id === configWidgetId}
            dragOver={dragOverId === w.id}
            onClick={editing ? () => setConfigWidgetId(w.id) : undefined}
            onConfigure={() => setConfigWidgetId(w.id)}
            onRemove={() => removeWidget(w.id)}
            onResize={() => cycleSize(w.id)}
            onRename={(name) => updateWidget(w.id, { title: name })}
            draggable={editing}
            onDragStart={onDragStart(w.id)}
            onDragOver={onDragOver(w.id)}
            onDragLeave={onDragLeave}
            onDragEnd={onDragEnd}
            onDrop={onDrop(w.id)}
          />
        ))}

        {editing && (
          <button
            className="widget placeholder span-1"
            onClick={() => setLibraryOpen(true)}
            type="button"
          >
            <div className="ph-content">
              <I.Plus width="22" height="22" />
              <div className="label">Add widget</div>
            </div>
          </button>
        )}
      </div>

      {/* Empty state */}
      {active.widgets.length === 0 && !editing && (
        <EmptyDashboard onAdd={() => { setEditing(true); setLibraryOpen(true); }} />
      )}

      {/* Floating edit footer */}
      {editing && (
        <div className="reports-edit-foot">
          <button className="btn ghost sm" onClick={resetAll}>
            <I.Refresh width="13" height="13" /> Reset all
          </button>
          <div className="muted" style={{ fontSize: 11.5, fontFamily: "var(--mono)", letterSpacing: 0.04 + "em" }}>
            DRAG TO REORDER · CLICK TO CONFIGURE · SAVES LOCALLY
          </div>
          <div style={{ display: "flex", gap: 6 }}>
            <button className="btn outline sm" onClick={() => setLibraryOpen(true)}>
              <I.Plus width="13" height="13" /> Widget
            </button>
            <button className="btn primary sm" onClick={() => { setEditing(false); setConfigWidgetId(null); }}>
              Done
            </button>
          </div>
        </div>
      )}

      {/* Config drawer */}
      {configWidget && editing && (
        <WidgetConfig
          widget={configWidget}
          onChange={(patch) => updateWidget(configWidget.id, patch)}
          onClose={() => setConfigWidgetId(null)}
          onRemove={() => removeWidget(configWidget.id)}
        />
      )}

      {/* Library modal */}
      {libraryOpen && (
        <WidgetLibrary
          onPick={addWidget}
          onClose={() => setLibraryOpen(false)}
        />
      )}
    </div>
  );
}

/* ────────────────────────────────────────────────────────
   ReportTabs — saved reports / dashboards along the top.
   Inline rename, duplicate, delete in edit mode.
   ──────────────────────────────────────────────────────── */
function ReportTabs({ reports, activeId, setActiveId, editing, renamingId, setRenamingId, onRename, onNew, onDuplicate, onDelete }) {
  return (
    <div className="reports-tabs">
      {reports.map(r => (
        <div
          key={r.id}
          className={`rtab ${r.id === activeId ? "active" : ""}`}
          onClick={() => setActiveId(r.id)}
          onDoubleClick={() => editing && setRenamingId(r.id)}
        >
          <span className="icon" aria-hidden>{r.icon || "◐"}</span>
          {renamingId === r.id ? (
            <input
              autoFocus
              className="rtab-name-input"
              defaultValue={r.name}
              onClick={(e) => e.stopPropagation()}
              onBlur={(e) => {
                onRename(r.id, e.target.value.trim() || r.name);
                setRenamingId(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.target.blur();
                if (e.key === "Escape") { e.target.value = r.name; e.target.blur(); }
              }}
            />
          ) : (
            <span>{r.name}</span>
          )}
          {editing && r.id === activeId && renamingId !== r.id && (
            <span className="rtab-actions">
              <button title="Rename" onClick={(e) => { e.stopPropagation(); setRenamingId(r.id); }}>
                <I.Pencil width="11" height="11" />
              </button>
              <button title="Duplicate" onClick={(e) => { e.stopPropagation(); onDuplicate(r.id); }}>
                <I.Copy width="11" height="11" />
              </button>
              {reports.length > 1 && (
                <button title="Delete" className="danger" onClick={(e) => {
                  e.stopPropagation();
                  if (confirm(`Delete "${r.name}"? This can't be undone.`)) onDelete(r.id);
                }}>
                  <I.Trash width="11" height="11" />
                </button>
              )}
            </span>
          )}
        </div>
      ))}
      {editing && (
        <button className="rtab-add" onClick={onNew}>
          <I.Plus width="12" height="12" /> NEW REPORT
        </button>
      )}
    </div>
  );
}

/* ────────────────────────────────────────────────────────
   WidgetShell — the card chrome around any widget's
   content. Adds drag handle, gear, resize, remove in edit
   mode. Click-to-configure. Inline title rename.
   ──────────────────────────────────────────────────────── */
function WidgetShell({
  widget, scope, editing, configuring, dragOver, draggable,
  onClick, onConfigure, onRemove, onResize, onRename,
  onDragStart, onDragOver, onDragLeave, onDragEnd, onDrop
}) {
  const [renaming, setRenaming] = React.useState(false);
  const tallClass = widget.h >= 3 ? "tall-3" : widget.h >= 2 ? "tall-2" : "";
  const useFlush = ["tableCat", "tableMer"].includes(widget.type);

  return (
    <div
      data-widget-id={widget.id}
      className={`widget span-${widget.w} ${tallClass} ${configuring ? "configuring" : ""} ${dragOver ? "drop-target" : ""}`}
      draggable={draggable && !renaming}
      onDragStart={onDragStart}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDragEnd={onDragEnd}
      onDrop={onDrop}
      onClick={(e) => {
        if (renaming) return;
        if (e.target.closest("button, input, textarea, select, a")) return;
        onClick && onClick();
      }}
    >
      {editing && (
        <>
          <div className="widget-handle" title="Drag to reorder">
            <I.Drag width="13" height="13" />
          </div>
          <div className="widget-chrome" onClick={(e) => e.stopPropagation()}>
            <span className="widget-size">{widget.w}/4</span>
            <button title="Cycle width" onClick={onResize}><I.Maximize width="13" height="13" /></button>
            <span className="sep"></span>
            <button title="Configure" onClick={onConfigure}><I.Gear width="13" height="13" /></button>
            <span className="sep"></span>
            <button title="Remove" className="danger" onClick={() => {
              if (confirm("Remove this widget?")) onRemove();
            }}>
              <I.Trash width="13" height="13" />
            </button>
          </div>
        </>
      )}

      <div className="widget-head">
        <div style={{ flex: 1, minWidth: 0 }}>
          {editing && renaming ? (
            <input
              autoFocus
              className="widget-title-input"
              defaultValue={widget.title}
              onClick={(e) => e.stopPropagation()}
              onBlur={(e) => {
                onRename(e.target.value.trim() || widget.title);
                setRenaming(false);
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") e.target.blur();
                if (e.key === "Escape") { e.target.value = widget.title; e.target.blur(); }
              }}
            />
          ) : (
            <>
              <div
                className="widget-title"
                title={editing ? "Click to rename" : undefined}
                onClick={(e) => { if (editing) { e.stopPropagation(); setRenaming(true); } }}
              >{widget.title}</div>
              {widget.config?.subtitle && (
                <div className="widget-sub">{widget.config.subtitle}</div>
              )}
            </>
          )}
        </div>
      </div>

      <div className={`widget-body ${useFlush ? "flush" : ""}`}>
        <WidgetContent widget={widget} scope={scope} />
      </div>
    </div>
  );
}

window.Reports = Reports;
