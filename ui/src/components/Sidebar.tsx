import { NavLink, useNavigate } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { prefetchRoute } from "../api/prefetch";
import * as I from "./Icons";
import { useAgentStatus, useNeedsReviewCount } from "../api/hooks/agent";
import { useResetOnboarding } from "../api/hooks/onboarding";
import { useAccounts } from "../api/hooks/accounts";
import { useGoals } from "../api/hooks/budget";
import { commands } from "../api/client";
import { isBackendAvailable } from "../utils/runtime";

interface NavEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

interface NavSection {
  /** null → the top "overview" group, rendered without a header. */
  label: string | null;
  items: NavEntry[];
}

// Grouped IA: a flat 13-item scroll became four scannable sections that read
// as a financial story — where you stand, where money lives, where you're
// headed, and the tools that run underneath. Every route/badge/pulse is
// preserved; only the grouping and visual hierarchy changed.
const NAV: NavSection[] = [
  {
    label: null,
    items: [
      { id: "today", path: "/", label: "Today", Icon: I.Today },
      { id: "inbox", path: "/inbox", label: "Inbox", Icon: I.Bell },
    ],
  },
  {
    label: "Money",
    items: [
      { id: "accounts", path: "/accounts", label: "Accounts", Icon: I.Wallet },
      { id: "budget", path: "/budget", label: "Budget", Icon: I.Lego },
      { id: "categories", path: "/categories", label: "Categories", Icon: I.Grid },
      { id: "recurring", path: "/recurring", label: "Recurring", Icon: I.Repeat },
    ],
  },
  {
    label: "Plan",
    items: [
      { id: "goals", path: "/goals", label: "Goals", Icon: I.Goal },
      { id: "reports", path: "/reports", label: "Reports", Icon: I.Spark },
      { id: "scenarios", path: "/scenarios", label: "Scenarios", Icon: I.Bolt },
      { id: "path-back", path: "/path-back", label: "Path back", Icon: I.Flow },
    ],
  },
  {
    label: "Workshop",
    items: [
      { id: "copilot", path: "/copilot", label: "Copilot", Icon: I.Brain },
      { id: "rules", path: "/rules", label: "Rules & agents", Icon: I.Bolt },
      { id: "settings", path: "/settings", label: "Settings", Icon: I.Gear },
    ],
  },
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
  const resetOnboarding = useResetOnboarding();
  const hasBackend = isBackendAvailable();
  const qc = useQueryClient();
  // Warm a route's summary queries the moment the user signals intent (hover /
  // keyboard focus), so the click paints from a warm cache. Idempotent + reads
  // only — safe to fire on every hover.
  const warm = (path: string) => prefetchRoute(qc, path);

  const { data: pendingBundles = [] } = useQuery({
    queryKey: ["action-bundles", "pending", null],
    queryFn: async () => {
      const result = await commands.listActionBundles("pending", null, null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
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
    return null;
  };

  const renderPulse = (id: string) => {
    if (id === "inbox" && (needsReview > 0 || hasAgentActivity)) return <span className="pulse" />;
    if (id === "rules" && needsReview > 0) return <span className="pulse" />;
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
        <I.Down className="ico" style={{ color: "var(--ink-faint)" }} aria-hidden="true" />
      </div>

      <button
        className="search-trigger"
        onClick={onOpenCmd}
        aria-label="Open command palette"
        type="button"
      >
        <I.Search width="14" height="14" style={{ color: "var(--ink-faint)" }} aria-hidden="true" />
        <span className="ph">Search or ask…</span>
        <span className="kbd">⌘K</span>
      </button>

      <nav className="nav" aria-label="Main">
        {NAV.map((section) => (
          <div key={section.label ?? "overview"} className="nav-group" role="group" aria-label={section.label ?? "Overview"}>
            {section.label && <div className="nav-section">{section.label}</div>}
            {section.items.map((n) => (
              <NavLink
                key={n.id}
                to={n.path}
                end={n.path === "/"}
                onMouseEnter={() => warm(n.path)}
                onFocus={() => warm(n.path)}
                className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
              >
                <span className="nav-ico-wrap" aria-hidden="true"><n.Icon className="ico" /></span>
                <span className="nav-label">{n.label}</span>
                <span className="nav-meta">
                  {renderPulse(n.id)}
                  {renderBadge(n.id)}
                </span>
              </NavLink>
            ))}
          </div>
        ))}
      </nav>

      <div className="foot">
        <button type="button" className="nav-item ghost" onClick={() => void handleRunSetup()}>
          <I.Sparkle className="ico" aria-hidden="true" />
          <span>Run setup again</span>
        </button>
        <div className="nav-item trust" aria-hidden="false">
          <I.Lock className="ico" aria-hidden="true" />
          <span>Local-only · encrypted</span>
        </div>
      </div>
    </aside>
  );
}
