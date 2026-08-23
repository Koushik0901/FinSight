import { NavLink } from "react-router-dom";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { prefetchRoute } from "../api/prefetch";
import * as I from "./Icons";
import { useAgentStatus, useNeedsReviewCount } from "../api/hooks/agent";
import { commands } from "../api/client";
import { unwrap } from "../api/client";
import { actionBundleKeys } from "../api/hooks/copilot";
import { isBackendAvailable } from "../utils/runtime";
import Drawer from "./Drawer";

interface TabEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

const TABS: TabEntry[] = [
  { id: "today", path: "/", label: "Today", Icon: I.Today },
  { id: "accounts", path: "/accounts", label: "Accounts", Icon: I.Wallet },
  { id: "budget", path: "/budget", label: "Budget", Icon: I.Lego },
  { id: "goals", path: "/goals", label: "Goals", Icon: I.Goal },
  { id: "copilot", path: "/copilot", label: "Copilot", Icon: I.Brain },
];

interface MoreEntry {
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

const MORE_ITEMS: MoreEntry[] = [
  { path: "/inbox", label: "Review", Icon: I.Check },
  { path: "/categories", label: "Categories", Icon: I.Grid },
  { path: "/recurring", label: "Recurring", Icon: I.Repeat },
  { path: "/cashflow", label: "Cash flow", Icon: I.Horizon },
  { path: "/reports", label: "Reports", Icon: I.Spark },
  { path: "/scenarios", label: "Scenarios", Icon: I.Bolt },
  { path: "/path-back", label: "Recovery plan", Icon: I.Flow },
  { path: "/journey", label: "Journey", Icon: I.Journey },
  { path: "/rules", label: "Rules & automation", Icon: I.Bolt },
  { path: "/recipes", label: "Recipes", Icon: I.Recipe },
  { path: "/settings", label: "Settings", Icon: I.Gear },
];

export function BottomNav() {
  const [moreOpen, setMoreOpen] = useState(false);
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const hasBackend = isBackendAvailable();
  const qc = useQueryClient();
  const warm = (path: string) => prefetchRoute(qc, path);

  const { data: pendingBundles = [] } = useQuery({
    queryKey: actionBundleKeys.list("pending"),
    queryFn: async () => {
      return unwrap(commands.listActionBundles("pending", null, null));
    },
    staleTime: 60_000,
    enabled: hasBackend,
  });

  const hasAgentActivity = Boolean(agentStatus?.lastScanAt || pendingBundles.length > 0);
  const reviewPulse = needsReview > 0 || hasAgentActivity;

  return (
    <>
      <nav className="bottom-nav" aria-label="Primary navigation (mobile)">
        {TABS.map((t) => (
          <NavLink
            key={t.id}
            to={t.path}
            end={t.path === "/"}
            aria-label={t.label}
            onMouseEnter={() => warm(t.path)}
            onFocus={() => warm(t.path)}
            className={({ isActive }) => `bottom-nav-item${isActive ? " active" : ""}`}
          >
            <span className="bottom-nav-ico-wrap" aria-hidden="true">
              <t.Icon className="ico" />
            </span>
            <span className="bottom-nav-label">{t.label}</span>
          </NavLink>
        ))}
        <button
          type="button"
          className="bottom-nav-item"
          onClick={() => setMoreOpen(true)}
          aria-haspopup="dialog"
          aria-expanded={moreOpen}
          aria-label="More"
        >
          <span className="bottom-nav-ico-wrap" aria-hidden="true">
            <I.More className="ico" />
            {reviewPulse && <span className="pulse" data-testid="bottom-nav-review-pulse" />}
          </span>
          <span className="bottom-nav-label">More</span>
        </button>
      </nav>

      <Drawer open={moreOpen} onClose={() => setMoreOpen(false)} title="More">
        <nav className="nav" aria-label="More destinations">
          {MORE_ITEMS.map((item) => (
            <NavLink
              key={item.path}
              to={item.path}
              onClick={() => setMoreOpen(false)}
              className={({ isActive }) => `nav-item${isActive ? " active" : ""}`}
            >
              <span className="nav-ico-wrap" aria-hidden="true">
                <item.Icon className="ico" />
              </span>
              <span className="nav-label">{item.label}</span>
            </NavLink>
          ))}
        </nav>
      </Drawer>
    </>
  );
}

export default BottomNav;
