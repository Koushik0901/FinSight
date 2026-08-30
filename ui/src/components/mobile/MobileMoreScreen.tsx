import { NavLink } from "react-router-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { prefetchRoute } from "../../api/prefetch";
import * as I from "../Icons";
import { useAgentStatus, useNeedsReviewCount } from "../../api/hooks/agent";
import { api } from "../../api/openapiClient";
import { unwrap } from "../../api/openapiClient";
import { actionBundleKeys } from "../../api/hooks/_factory";
import { isBackendAvailable } from "../../utils/runtime";

interface MoreEntry {
  path: string;
  label: string;
  description: string;
  Icon: React.FC<React.SVGProps<SVGSVGElement>>;
}

interface MoreGroup {
  title: string;
  items: MoreEntry[];
}

const MORE_GROUPS: MoreGroup[] = [
  {
    title: "Money",
    items: [
      { path: "/accounts", label: "Accounts", description: "Balances & holdings", Icon: I.Wallet },
      { path: "/goals", label: "Goals", description: "Targets & progress", Icon: I.Goal },
      { path: "/recurring", label: "Recurring", description: "Bills & income", Icon: I.Repeat },
      { path: "/categories", label: "Categories", description: "Spending breakdown", Icon: I.Grid },
    ],
  },
  {
    title: "Plan",
    items: [
      { path: "/inbox", label: "Review", description: "Needs attention", Icon: I.Check },
      { path: "/cashflow", label: "Cash flow", description: "Upcoming flow", Icon: I.Horizon },
      { path: "/reports", label: "Reports", description: "History & trends", Icon: I.Spark },
      { path: "/scenarios", label: "Scenarios", description: "What-if planner", Icon: I.Bolt },
      { path: "/path-back", label: "Recovery plan", description: "Get back on track", Icon: I.Flow },
      { path: "/journey", label: "Journey", description: "Milestones", Icon: I.Journey },
    ],
  },
  {
    title: "System",
    items: [
      { path: "/rules", label: "Rules & automation", description: "Categorization", Icon: I.Bolt },
      { path: "/recipes", label: "Recipes", description: "Agent recipes", Icon: I.Recipe },
      { path: "/settings", label: "Settings", description: "App & account", Icon: I.Gear },
    ],
  },
];

export function MobileMoreScreen() {
  const qc = useQueryClient();
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const { data: agentStatus } = useAgentStatus();
  const hasBackend = isBackendAvailable();

  const { data: pendingBundles = [] } = useQuery({
    queryKey: actionBundleKeys.list("pending"),
    queryFn: async () => unwrap(api.listActionBundles("pending", null, null)),
    staleTime: 60_000,
    enabled: hasBackend,
  });

  const hasAgentActivity = Boolean(agentStatus?.lastScanAt || pendingBundles.length > 0);
  const showReviewBadge = needsReview > 0 || hasAgentActivity;
  const warm = (path: string) => prefetchRoute(qc, path);

  return (
    <div className="mobile-more" role="main" aria-label="More">
      <div className="mobile-more-hero">
        <h1>More</h1>
        <p>Accounts, goals, and tools — everything beyond the four main tabs.</p>
      </div>

      {MORE_GROUPS.map((group) => (
        <section key={group.title} className="mobile-more-group" aria-label={group.title}>
          <h2 className="mobile-more-group-title">{group.title}</h2>
          <div className="mobile-more-list">
            {group.items.map((item) => {
              const isReview = item.path === "/inbox";
              return (
                <NavLink
                  key={item.path}
                  to={item.path}
                  onMouseEnter={() => warm(item.path)}
                  onFocus={() => warm(item.path)}
                  className="mobile-more-row"
                >
                  <span className="mobile-more-row-ico" aria-hidden="true">
                    <item.Icon width={16} height={16} />
                  </span>
                  <span className="mobile-more-row-label">
                    <span style={{ display: "block", fontWeight: 600, lineHeight: 1.2 }}>{item.label}</span>
                    <span style={{ display: "block", fontSize: 12, color: "var(--ink-mute)", lineHeight: 1.3 }}>
                      {item.description}
                    </span>
                  </span>
                  {isReview && showReviewBadge ? (
                    <span className="mobile-more-row-meta" data-testid="more-review-pulse">
                      <span
                        style={{
                          width: 8,
                          height: 8,
                          borderRadius: "50%",
                          background: "var(--accent)",
                          display: "inline-block",
                        }}
                      />
                      Action needed
                    </span>
                  ) : null}
                  <span className="mobile-more-row-chevron" aria-hidden="true">
                    <I.ArrowRight width={14} height={14} />
                  </span>
                </NavLink>
              );
            })}
          </div>
        </section>
      ))}
    </div>
  );
}

export default MobileMoreScreen;
