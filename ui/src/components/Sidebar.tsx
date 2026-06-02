import { NavLink } from "react-router-dom";
import * as I from "./Icons";
import { useNeedsReviewCount } from "../api/hooks/agent";

interface NavEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
  badge?: string;
  pulse?: boolean;
}

const NAV_MAIN: NavEntry[] = [
  { id: "today",        path: "/",              label: "Today",        Icon: I.Today },
  { id: "insights",     path: "/insights",      label: "Insights",     Icon: I.Sparkle },
  { id: "accounts",     path: "/accounts",      label: "Accounts",     Icon: I.Wallet },
  { id: "transactions", path: "/transactions",  label: "Transactions", Icon: I.Flow },
  { id: "budget",       path: "/budget",        label: "Budget",       Icon: I.Lego },
  { id: "categories",   path: "/categories",    label: "Categories",   Icon: I.Grid },
  { id: "recurring",    path: "/recurring",     label: "Recurring",    Icon: I.Repeat },
  { id: "goals",        path: "/goals",         label: "Goals",        Icon: I.Goal },
  { id: "reports",      path: "/reports",       label: "Reports",      Icon: I.Spark },
];

const NAV_WORKSHOP: NavEntry[] = [
  { id: "rules",    path: "/rules",    label: "Rules & agents", Icon: I.Bolt },
  { id: "settings", path: "/settings", label: "Settings",       Icon: I.Gear },
];

interface Props {
  onOpenCmd: () => void;
}

export function Sidebar({ onOpenCmd }: Props) {
  const { data: needsReview = 0 } = useNeedsReviewCount();

  return (
    <aside className="sidebar" aria-label="Primary navigation">
      {/* Brand */}
      <div className="brand">
        <div className="mark" aria-hidden="true" />
        <div className="wm">FinSight</div>
      </div>

      {/* Search / command palette trigger */}
      <button
        className="search-trigger"
        onClick={onOpenCmd}
        aria-label="Open command palette"
      >
        <I.Search width="14" height="14" style={{ color: "var(--ink-faint)" }} />
        <span className="ph">Search or ask…</span>
        <span className="kbd">⌘K</span>
      </button>

      {/* Main navigation */}
      <nav className="nav">
        {NAV_MAIN.map((n) => (
          <NavLink
            key={n.id}
            to={n.path}
            end={n.path === "/"}
            className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
          >
            <n.Icon className="ico" />
            <span>{n.label}</span>
            {n.id === "rules" && needsReview > 0 && (
              <span className="pulse" title={`${needsReview} need review`} />
            )}
            {n.badge && <span className="badge">{n.badge}</span>}
          </NavLink>
        ))}

        <div className="nav-section">Workshop</div>
        {NAV_WORKSHOP.map((n) => (
          <NavLink
            key={n.id}
            to={n.path}
            end
            className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
          >
            <n.Icon className="ico" />
            <span>{n.label}</span>
            {n.id === "rules" && needsReview > 0 && (
              <span className="pulse" title={`${needsReview} need review`} />
            )}
          </NavLink>
        ))}
      </nav>

      {/* Footer */}
      <div className="foot">
        <div
          className="nav-item"
          style={{ color: "var(--ink-faint)", cursor: "default" }}
        >
          <I.Lock className="ico" />
          <span style={{ fontSize: 12 }}>Local-only · encrypted</span>
        </div>
      </div>
    </aside>
  );
}
