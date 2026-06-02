/* Small reusable primitives */

function Sparkline({ values, color = "currentColor", height = 28, width = 100, fill = false }) {
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const points = values.map((v, i) => {
    const x = (i / (values.length - 1)) * width;
    const y = height - ((v - min) / range) * (height - 4) - 2;
    return [x, y];
  });
  const d = points.map((p, i) => (i === 0 ? `M${p[0]},${p[1]}` : `L${p[0]},${p[1]}`)).join(" ");
  const area = `${d} L${width},${height} L0,${height} Z`;
  return (
    <svg viewBox={`0 0 ${width} ${height}`} width="100%" height={height} preserveAspectRatio="none">
      {fill && <path d={area} fill={color} opacity="0.12" />}
      <path d={d} fill="none" stroke={color} strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

function Currency({ value, signed = false, decimals = 0, className = "" }) {
  return <span className={`num tabular ${value < 0 ? "neg" : ""} ${className}`}>{FS.fmt(value, { signed, decimals })}</span>;
}

function Delta({ value, suffix = "" }) {
  const positive = value >= 0;
  return (
    <span className={`num tabular ${positive ? "pos" : "neg"}`} style={{ fontSize: 13 }}>
      {positive ? "▲" : "▼"} {FS.fmt(Math.abs(value), { decimals: 0 })}{suffix}
    </span>
  );
}

function SectionHeader({ eyebrow, title, action }) {
  return (
    <div style={{ display: "flex", alignItems: "flex-end", justifyContent: "space-between", marginBottom: 18 }}>
      <div>
        {eyebrow && <div className="eyebrow"><span className="dot"></span>{eyebrow}</div>}
        {title && <h2 className="h1" style={{ marginTop: 8 }}>{title}</h2>}
      </div>
      {action}
    </div>
  );
}

function Stat({ label, value, sub, accent }) {
  return (
    <div className={`stat ${accent ? "accent" : ""}`}>
      <div className="label">{label}</div>
      <div className="value">{value}</div>
      {sub && <div className="sub">{sub}</div>}
    </div>
  );
}

Object.assign(window, { Sparkline, Currency, Delta, SectionHeader, Stat });

/* ────────────────────────────────────────────────────────
   Toast — global feedback for any action across the app.
   Use:   window.toast("Saved.");
          window.toast("Removed Disney+", { kind: "warn" });
          window.toast("Couldn't reach bank", { kind: "error" });
   ──────────────────────────────────────────────────────── */
function Toaster() {
  const [toasts, setToasts] = React.useState([]);

  React.useEffect(() => {
    window.toast = (msg, opts = {}) => {
      const id = "t_" + Math.random().toString(36).slice(2, 8);
      const t = {
        id,
        msg,
        kind: opts.kind || "info",
        sub: opts.sub,
        action: opts.action,        // { label, onClick }
      };
      setToasts(prev => [...prev.slice(-2), t]);   // cap at 3 visible
      const dur = opts.duration || (opts.action ? 5000 : 2800);
      setTimeout(() => setToasts(prev => prev.filter(x => x.id !== id)), dur);
      return id;
    };
    return () => { delete window.toast; };
  }, []);

  return (
    <div className="toast-host" aria-live="polite">
      {toasts.map(t => (
        <div key={t.id} className={`toast ${t.kind}`}>
          <span className="dot"></span>
          <div className="toast-body">
            <div className="msg">{t.msg}</div>
            {t.sub && <div className="sub">{t.sub}</div>}
          </div>
          {t.action && (
            <button
              className="toast-action"
              onClick={() => {
                t.action.onClick?.();
                setToasts(prev => prev.filter(x => x.id !== t.id));
              }}
            >
              {t.action.label}
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

window.Toaster = Toaster;
