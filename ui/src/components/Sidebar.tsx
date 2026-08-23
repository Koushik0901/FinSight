import { useEffect, useState } from "react";
import { NavLink, useLocation, useNavigate } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { prefetchRoute } from "../api/prefetch";
import * as I from "./Icons";
import { useAgentStatus, useNeedsReviewCount } from "../api/hooks/agent";
import { useResetOnboarding } from "../api/hooks/onboarding";
import { useAccounts } from "../api/hooks/accounts";
import { useGoals } from "../api/hooks/budget";
import { api } from "../api/openapiClient";
import { unwrap } from "../api/openapiClient";
import { actionBundleKeys } from "../api/hooks/_factory";
import { isBackendAvailable } from "../utils/runtime";

interface NavEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

const CORE_NAV: NavEntry[] = [
  { id: "today", path: "/", label: "Today", Icon: I.Today },
  { id: "accounts", path: "/accounts", label: "Accounts", Icon: I.Wallet },
  { id: "budget", path: "/budget", label: "Budget", Icon: I.Lego },
  { id: "goals", path: "/goals", label: "Goals", Icon: I.Goal },
  { id: "reports", path: "/reports", label: "Reports", Icon: I.Spark },
  { id: "copilot", path: "/copilot", label: "Copilot", Icon: I.Brain },
];

const MORE_NAV: NavEntry[] = [
  { id: "review", path: "/inbox", label: "Review", Icon: I.Check },
  { id: "categories", path: "/categories", label: "Categories", Icon: I.Grid },
  { id: "recurring", path: "/recurring", label: "Recurring", Icon: I.Repeat },
  { id: "cashflow", path: "/cashflow", label: "Cash flow", Icon: I.Horizon },
  { id: "scenarios", path: "/scenarios", label: "Scenarios", Icon: I.Bolt },
  { id: "path-back", path: "/path-back", label: "Recovery plan", Icon: I.Flow },
  { id: "journey", path: "/journey", label: "Journey", Icon: I.Journey },
  { id: "rules", path: "/rules", label: "Rules & automation", Icon: I.Bolt },
  { id: "recipes", path: "/recipes", label: "Recipes", Icon: I.Recipe },
];

interface Props {
  onOpenCmd: () => void;
}

export function Sidebar({ onOpenCmd }: Props) {
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const { data: accounts = [] } = useAccounts();
  const { data: goals = [] } = useGoals();
  const navigate = useNavigate();
  const location = useLocation();
  const moreActive = MORE_NAV.some((item) => location.pathname === item.path || location.pathname.startsWith(`${item.path}/`));
  const [moreOpen, setMoreOpen] = useState(moreActive);
  useEffect(() => { if (moreActive) setMoreOpen(true); }, [moreActive]);
  const resetOnboarding = useResetOnboarding();
  const hasBackend = isBackendAvailable();
  const qc = useQueryClient();
  // Warm a route's summary queries the moment the user signals intent (hover /
  // keyboard focus), so the click paints from a warm cache. Idempotent + reads
  // only — safe to fire on every hover.
  const warm = (path: string) => prefetchRoute(qc, path);

  const { data: pendingBundles = [] } = useQuery({
    queryKey: actionBundleKeys.list("pending"),
    queryFn: async () => {
      return unwrap(api.listActionBundles("pending", null, null));
    },
    staleTime: 60_000,
    enabled: hasBackend,
  });

  const pendingBundleCount = pendingBundles.length;
  const leadAvatar = (accounts[0]?.name?.trim().slice(0, 1) || "Y").toUpperCase();
  const altAvatar = (accounts[1]?.name?.trim().slice(0, 1) || "F").toUpperCase();
  const profileLabel = accounts.length > 1 ? "Household" : "Personal";
  const hasAgentActivity = Boolean(agentStatus?.lastScanAt || pendingBundleCount > 0);

  const handleRunSetup = async () => {
    try {
      await resetOnboarding.mutateAsync();
      navigate("/onboarding");
    } catch {
      toast.error("Failed to reset setup");
    }
  };

  const renderBadge = (id: string) => {
    if (id === "accounts" && accounts.length > 0) return <span className="badge">{accounts.length}</span>;
    if (id === "goals" && goals.length > 0) return <span className="badge">{goals.length}</span>;
    if (id === "copilot" && pendingBundleCount > 0) return <span className="badge accent">{pendingBundleCount}</span>;
    // `needsReview` is the pending-proposal count, so it belongs on the screen
    // that actually clears them.
    if (id === "review" && needsReview > 0) return <span className="badge accent">{needsReview}</span>;
    return null;
  };

  const renderPulse = (id: string) => {
    if (id === "review" && (needsReview > 0 || hasAgentActivity)) return <span className="pulse" />;
    return null;
  };

  return (
    <aside className="sidebar" aria-label="Primary navigation">
      <div className="brand">
        <div className="mark" aria-hidden="true" />
        <div className="wm">FinSight</div>
      </div>

      <div className="who" aria-label={`${profileLabel} workspace`}>
        <div className="stack" aria-hidden="true">
          <div className="av">{leadAvatar}</div>
          <div className="av b">{altAvatar}</div>
        </div>
        <div className="meta">
          <div className="name">Your workspace</div>
          <div className="sub">
            {profileLabel} · {accounts.length} account{accounts.length === 1 ? "" : "s"}
          </div>
        </div>
      </div>

      <button
        className="search-trigger"
        onClick={onOpenCmd}
        aria-label="Open command palette"
        type="button"
      >
        <I.Search width="14" height="14" style={{ color: "var(--ink-faint)" }} aria-hidden="true" />
        <span className="ph">Find or ask…</span>
        <span className="kbd">⌘K</span>
      </button>

      <nav className="nav" aria-label="Main">
        <div className="nav-group" role="group" aria-label="Core destinations">
          {CORE_NAV.map((n) => (
            <NavLink key={n.id} to={n.path} end={n.path === "/"} onMouseEnter={() => warm(n.path)} onFocus={() => warm(n.path)} className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}>
              <span className="nav-ico-wrap" aria-hidden="true"><n.Icon className="ico" /></span>
              <span className="nav-label">{n.label}</span>
              <span className="nav-meta">{renderBadge(n.id)}</span>
            </NavLink>
          ))}
        </div>
        <button type="button" className={`nav-item nav-more-toggle${moreActive ? " active" : ""}`} aria-expanded={moreOpen} aria-controls="sidebar-more" onClick={() => setMoreOpen((open) => !open)}>
          <span className="nav-ico-wrap" aria-hidden="true"><I.More className="ico" /></span>
          <span className="nav-label">More</span>
          <span className="nav-meta">{renderPulse("review")}{renderBadge("review")}<I.Down className="ico nav-more-chevron" aria-hidden="true" /></span>
        </button>
        {moreOpen && (
          <div id="sidebar-more" className="nav-group nav-more" role="group" aria-label="More destinations">
            {MORE_NAV.map((n) => (
              <NavLink key={n.id} to={n.path} onMouseEnter={() => warm(n.path)} onFocus={() => warm(n.path)} className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}>
                <span className="nav-ico-wrap" aria-hidden="true"><n.Icon className="ico" /></span>
                <span className="nav-label">{n.label}</span>
                <span className="nav-meta">{renderPulse(n.id)}{renderBadge(n.id)}</span>
              </NavLink>
            ))}
          </div>
        )}
      </nav>

      <div className="foot">
        <NavLink to="/settings" className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}>
          <I.Gear className="ico" aria-hidden="true" />
          <span>Settings</span>
        </NavLink>
        <button type="button" className="nav-item ghost" onClick={() => void handleRunSetup()}>
          <I.Sparkle className="ico" aria-hidden="true" />
          <span>{accounts.length === 0 ? "Finish setup" : "Setup & import"}</span>
        </button>
        <div className="nav-item trust" aria-hidden="false">
          <I.Lock className="ico" aria-hidden="true" />
          <span>Local-only · encrypted</span>
        </div>
      </div>
    </aside>
  );
}
