import { NavLink, useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { toast } from "sonner";
import * as I from "./Icons";
import { useNeedsReviewCount } from "../api/hooks/agent";
import { useResetOnboarding } from "../api/hooks/onboarding";
import { useActionItems } from "../api/hooks/inbox";
import { commands } from "../api/client";
import { isTauriRuntime } from "../utils/runtime";

interface NavEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

const NAV_MAIN: NavEntry[] = [
  { id: "today", path: "/", label: "Today", Icon: I.Today },
  { id: "inbox", path: "/inbox", label: "Inbox", Icon: I.Bell },
  { id: "copilot", path: "/copilot", label: "Copilot", Icon: I.Brain },
  { id: "journey", path: "/journey", label: "Journey", Icon: I.Journey },
  { id: "recipes", path: "/recipes", label: "Recipes", Icon: I.Recipe },
  { id: "insights", path: "/insights", label: "Insights", Icon: I.Sparkle },
  { id: "accounts", path: "/accounts", label: "Accounts", Icon: I.Wallet },
  { id: "transactions", path: "/transactions", label: "Transactions", Icon: I.Flow },
  { id: "budget", path: "/budget", label: "Budget", Icon: I.Lego },
  { id: "categories", path: "/categories", label: "Categories", Icon: I.Grid },
  { id: "recurring", path: "/recurring", label: "Recurring", Icon: I.Repeat },
  { id: "goals", path: "/goals", label: "Goals", Icon: I.Goal },
  { id: "scenarios", path: "/scenarios", label: "Scenarios", Icon: I.Bolt },
  { id: "reports", path: "/reports", label: "Reports", Icon: I.Spark },
];

const NAV_WORKSHOP: NavEntry[] = [
  { id: "rules", path: "/rules", label: "Rules & agents", Icon: I.Bolt },
  { id: "settings", path: "/settings", label: "Settings", Icon: I.Gear },
];

interface Props {
  onOpenCmd: () => void;
}

export function Sidebar({ onOpenCmd }: Props) {
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const navigate = useNavigate();
  const resetOnboarding = useResetOnboarding();
  const { data: actionItems = [] } = useActionItems();
  const highPriorityCount = actionItems.filter((i) => i.priority === "high").length;
  const canUseDesktopApi = isTauriRuntime();

  const { data: txnCount = 0 } = useQuery<number>({
    queryKey: ["transaction-count"],
    queryFn: async () => {
      const result = await commands.getTransactionCount();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    refetchInterval: 60_000,
    enabled: canUseDesktopApi,
  });

  const { data: pendingBundles = [] } = useQuery({
    queryKey: ["action-bundles", "pending", null],
    queryFn: async () => {
      const result = await commands.listActionBundles("pending", null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 30_000,
    refetchInterval: 60_000,
    enabled: canUseDesktopApi,
  });

  const pendingBundleCount = pendingBundles.length;

  const formattedTxnCount =
    txnCount >= 1000 ? `${(txnCount / 1000).toFixed(1)}k` : String(txnCount);

  const handleRunSetup = async () => {
    try {
      await resetOnboarding.mutateAsync();
      navigate("/onboarding");
    } catch {
      toast.error("Failed to reset setup");
    }
  };

  return (
    <aside className="sidebar" aria-label="Primary navigation">
      <div className="brand">
        <div className="mark" aria-hidden="true" />
        <div className="wm">FinSight</div>
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
        {NAV_MAIN.map((n) => (
          <NavLink
            key={n.id}
            to={n.path}
            end={n.path === "/"}
            className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
          >
            <n.Icon className="ico" aria-hidden="true" />
            <span>{n.label}</span>
            {n.id === "inbox" && highPriorityCount > 0 && (
              <span
                className="badge negative"
                style={{ marginLeft: "auto" }}
                title={`${highPriorityCount} high-priority item${highPriorityCount !== 1 ? "s" : ""}`}
              >
                {highPriorityCount}
              </span>
            )}
            {n.id === "copilot" && pendingBundleCount > 0 && (
              <span
                className="badge accent"
                style={{ marginLeft: "auto" }}
                title={`${pendingBundleCount} bundle${pendingBundleCount !== 1 ? "s" : ""} awaiting review`}
              >
                {pendingBundleCount}
              </span>
            )}
            {n.id === "transactions" && txnCount > 0 && (
              <span className="badge" style={{ marginLeft: "auto" }}>
                {formattedTxnCount}
              </span>
            )}
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
            <n.Icon className="ico" aria-hidden="true" />
            <span>{n.label}</span>
            {n.id === "rules" && needsReview > 0 && (
              <span className="pulse" title={`${needsReview} need review`} />
            )}
          </NavLink>
        ))}
      </nav>

      <div className="foot">
        <div className="nav-item trust" aria-hidden="false">
          <I.Lock className="ico" aria-hidden="true" />
          <span>Local-only · encrypted</span>
        </div>
        <button type="button" className="nav-item ghost" onClick={() => void handleRunSetup()}>
          <I.Sparkle className="ico" aria-hidden="true" />
          <span>Run setup again</span>
        </button>
      </div>
    </aside>
  );
}
