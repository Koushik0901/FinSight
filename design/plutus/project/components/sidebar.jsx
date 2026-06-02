function Sidebar({ route, setRoute, openCmd, openOnboarding }) {
  const nav = [
    { id: "today",        label: "Today",         icon: I.Today },
    { id: "insights",     label: "Insights",      icon: I.Sparkle,  pulse: true },
    { id: "accounts",     label: "Accounts",      icon: I.Wallet,   badge: "11" },
    { id: "transactions", label: "Transactions",  icon: I.Flow,     badge: "1.2k" },
    { id: "budget",       label: "Budget",        icon: I.Lego },
    { id: "categories",   label: "Categories",    icon: I.Grid },
    { id: "recurring",    label: "Recurring",     icon: I.Repeat },
    { id: "goals",        label: "Goals",         icon: I.Goal,     badge: "5" },
    { id: "scenarios",    label: "Scenarios",     icon: I.Bolt },
    { id: "reports",      label: "Reports",       icon: I.Spark },
  ];
  const power = [
    { id: "rules",    label: "Rules & agents", icon: I.Bolt },
    { id: "settings", label: "Settings",       icon: I.Gear },
  ];
  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="mark"></div>
        <div className="wm">FinSight</div>
      </div>

      <div className="who" title="Switch household">
        <div className="stack">
          <div className="av">M</div>
          <div className="av b">A</div>
        </div>
        <div className="meta">
          <div className="name">Mira &amp; Adam</div>
          <div className="sub">Household · 6 accounts</div>
        </div>
        <I.Down style={{ color: "var(--ink-faint)" }} />
      </div>

      <button
        className="search-trigger"
        onClick={openCmd}
      >
        <I.Search style={{ color: "var(--ink-faint)" }} />
        <span className="ph">Search or ask…</span>
        <span className="kbd" style={{ fontFamily: "var(--mono)" }}>⌘K</span>
      </button>

      <nav className="nav">
        {nav.map(n => {
          const Ico = n.icon;
          return (
            <div key={n.id} className={`nav-item ${route === n.id ? "active" : ""}`} onClick={() => setRoute(n.id)}>
              <Ico className="ico" />
              <span>{n.label}</span>
              {n.badge && !n.pulse && <span className="badge">{n.badge}</span>}
              {n.pulse && <span className="pulse" title="1 needs attention"></span>}
            </div>
          );
        })}

        <div className="nav-section">Workshop</div>
        {power.map(n => {
          const Ico = n.icon;
          return (
            <div key={n.id} className={`nav-item ${route === n.id ? "active" : ""}`} onClick={() => setRoute(n.id)}>
              <Ico className="ico" />
              <span>{n.label}</span>
            </div>
          );
        })}
      </nav>

      <div className="foot">
        <div className="nav-item" onClick={openOnboarding}>
          <I.Sparkle className="ico" />
          <span>Run setup again</span>
        </div>
        <div className="nav-item" style={{ color: "var(--ink-faint)" }}>
          <I.Lock className="ico" />
          <span>Local-only · synced 2m ago</span>
        </div>
      </div>
    </aside>
  );
}
window.Sidebar = Sidebar;
