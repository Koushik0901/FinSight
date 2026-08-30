import type { ReactNode } from "react";
import { useLocation } from "react-router-dom";
import MobileBottomNav from "./MobileBottomNav";
import MobileHeader from "./MobileHeader";

interface MobileShellProps {
  children: ReactNode;
}

/** Map route → header title (keeps header concise on phone — not every desktop PageHeader detail). */
function titleForPath(pathname: string): { title: string; eyebrow?: string } {
  if (pathname === "/") return { title: "Today", eyebrow: "FinSight" };
  if (pathname === "/transactions" || pathname.startsWith("/accounts/")) return { title: "Transactions" };
  if (pathname === "/copilot") return { title: "Copilot", eyebrow: "Ask anything" };
  if (pathname === "/budget") return { title: "Budget" };
  if (pathname === "/more") return { title: "More" };
  if (pathname.startsWith("/accounts")) return { title: "Accounts" };
  if (pathname.startsWith("/goals")) return { title: "Goals" };
  if (pathname.startsWith("/categories")) return { title: "Categories" };
  if (pathname.startsWith("/recurring")) return { title: "Recurring" };
  if (pathname.startsWith("/inbox") || pathname.startsWith("/insights")) return { title: "Review" };
  if (pathname.startsWith("/reports")) return { title: "Reports" };
  if (pathname.startsWith("/scenarios")) return { title: "Scenarios" };
  if (pathname.startsWith("/cashflow")) return { title: "Cash flow" };
  if (pathname.startsWith("/journey")) return { title: "Journey" };
  if (pathname.startsWith("/rules")) return { title: "Rules" };
  if (pathname.startsWith("/recipes")) return { title: "Recipes" };
  if (pathname.startsWith("/settings")) return { title: "Settings" };
  if (pathname.startsWith("/close")) return { title: "Month close" };
  if (pathname.startsWith("/path-back")) return { title: "Recovery plan" };
  return { title: "FinSight" };
}

export function MobileShell({ children }: MobileShellProps) {
  const location = useLocation();
  const { title, eyebrow } = titleForPath(location.pathname);

  // Copilot gets a chrome-less header — the chat itself owns its header/composer.
  const isCopilot = location.pathname.startsWith("/copilot");

  return (
    <div className="mobile-shell" data-testid="mobile-shell">
      {!isCopilot ? <MobileHeader title={title} eyebrow={eyebrow} /> : null}
      <main className="mobile-shell-main" id="main" tabIndex={-1}>
        <div className="mobile-shell-inner">{children}</div>
      </main>
      <MobileBottomNav />
    </div>
  );
}

export default MobileShell;
