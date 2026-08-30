import { NavLink, useLocation } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { prefetchRoute } from "../../api/prefetch";
import * as I from "../Icons";
import { useAgentStatus, useNeedsReviewCount } from "../../api/hooks/agent";
import { api } from "../../api/openapiClient";
import { unwrap } from "../../api/openapiClient";
import { actionBundleKeys } from "../../api/hooks/_factory";
import { isBackendAvailable } from "../../utils/runtime";

interface TabEntry {
  id: string;
  path: string;
  label: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

const TABS: TabEntry[] = [
  { id: "today", path: "/", label: "Today", Icon: I.Today },
  { id: "transactions", path: "/transactions", label: "Transactions", Icon: I.Grid },
  { id: "copilot", path: "/copilot", label: "Copilot", Icon: I.Brain },
  { id: "budget", path: "/budget", label: "Budget", Icon: I.Lego },
];

/** Destinations that live under "More" — used to mark More as active when inside them */
const MORE_PATHS = [
  "/accounts",
  "/goals",
  "/recurring",
  "/categories",
  "/inbox",
  "/insights",
  "/reports",
  "/scenarios",
  "/cashflow",
  "/close",
  "/path-back",
  "/journey",
  "/rules",
  "/recipes",
  "/settings",
  "/more",
];

export function MobileBottomNav() {
  const location = useLocation();
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const hasBackend = isBackendAvailable();
  const qc = useQueryClient();

  const warm = (path: string) => prefetchRoute(qc, path);

  const { data: pendingBundles = [] } = useQuery({
    queryKey: actionBundleKeys.list("pending"),
    queryFn: async () => unwrap(api.listActionBundles("pending", null, null)),
    staleTime: 60_000,
    enabled: hasBackend,
  });

  const hasAgentActivity = Boolean(agentStatus?.lastScanAt || pendingBundles.length > 0);
  const reviewPulse = needsReview > 0 || hasAgentActivity;

  const moreActive = MORE_PATHS.some(
    (p) => location.pathname === p || location.pathname.startsWith(`${p}/`)
  );

  return (
    <nav className="mobile-bottom-nav" aria-label="Primary navigation">
      {TABS.map((t) => (
        <NavLink
          key={t.id}
          to={t.path}
          end={t.path === "/"}
          aria-label={t.label}
          onMouseEnter={() => warm(t.path)}
          onFocus={() => warm(t.path)}
          className={({ isActive }) => `mobile-bottom-nav-item${isActive ? " active" : ""}`}
        >
          <span className="mobile-bottom-nav-ico" aria-hidden="true">
            <t.Icon width={18} height={18} aria-hidden="true" />
          </span>
          <span className="mobile-bottom-nav-label">{t.label}</span>
        </NavLink>
      ))}

      <NavLink
        to="/more"
        aria-label="More"
        onMouseEnter={() => warm("/more")}
        onFocus={() => warm("/more")}
        className={({ isActive }) =>
          `mobile-bottom-nav-item${isActive || moreActive ? " active" : ""}`
        }
      >
        <span className="mobile-bottom-nav-ico" aria-hidden="true">
          <I.More width={18} height={18} aria-hidden="true" />
          {reviewPulse ? <span className="mobile-bottom-nav-dot" data-testid="mobile-nav-review-pulse" /> : null}
        </span>
        <span className="mobile-bottom-nav-label">More</span>
      </NavLink>
    </nav>
  );
}

export default MobileBottomNav;
