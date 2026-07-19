import { NavLink } from "react-router-dom";
import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { prefetchRoute } from "../api/prefetch";
import * as I from "./Icons";
import { useAgentStatus, useNeedsReviewCount } from "../api/hooks/agent";
import { commands } from "../api/client";
import { isBackendAvailable } from "../utils/runtime";
import Drawer from "./Drawer";

interface TabEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

// The 5 highest-value destinations, mirroring Sidebar's "Overview"/"Money"/"Plan"
// groups. Everything else lives behind the "More" sheet below.
const TABS: TabEntry[] = [
  { id: "today", path: "/", label: "Today", Icon: I.Today },
  { id: "inbox", path: "/inbox", label: "Inbox", Icon: I.Bell },
  { id: "accounts", path: "/accounts", label: "Accounts", Icon: I.Wallet },
  { id: "budget", path: "/budget", label: "Budget", Icon: I.Lego },
  { id: "goals", path: "/goals", label: "Goals", Icon: I.Goal },
];

interface MoreEntry {
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

const MORE_ITEMS: MoreEntry[] = [
  { path: "/categories", label: "Categories", Icon: I.Grid },
  { path: "/recurring", label: "Recurring", Icon: I.Repeat },
  { path: "/reports", label: "Reports", Icon: I.Spark },
  { path: "/scenarios", label: "Scenarios", Icon: I.Bolt },
  { path: "/path-back", label: "Path back", Icon: I.Flow },
  { path: "/journey", label: "Journey", Icon: I.Journey },
  { path: "/copilot", label: "Copilot", Icon: I.Brain },
  { path: "/rules", label: "Rules & agents", Icon: I.Bolt },
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
    queryKey: ["action-bundles", "pending", null],
    queryFn: async () => {
      const result = await commands.listActionBundles("pending", null, null);
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    staleTime: 60_000,
    enabled: hasBackend,
  });

  const hasAgentActivity = Boolean(agentStatus?.lastScanAt || pendingBundles.length > 0);
  const inboxPulse = needsReview > 0 || hasAgentActivity;

  return (
    <>
      <nav className="bottom-nav" aria-label="Primary navigation (mobile)">
        {TABS.map((t) => (
          <NavLink
            key={t.id}
            to={t.path}
            end={t.path === "/"}
            onMouseEnter={() => warm(t.path)}
            onFocus={() => warm(t.path)}
            className={({ isActive }) => `bottom-nav-item${isActive ? " active" : ""}`}
          >
            <span className="bottom-nav-ico-wrap" aria-hidden="true">
              <t.Icon className="ico" />
              {t.id === "inbox" && inboxPulse && <span className="pulse" data-testid="bottom-nav-inbox-pulse" />}
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
        >
          <span className="bottom-nav-ico-wrap" aria-hidden="true">
            <I.More className="ico" />
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
