import {
  Component,
  Suspense,
  lazy,
  useEffect,
  useRef,
  useState,
  type ErrorInfo,
  type ReactNode,
} from "react";
import { Navigate, Route, Routes, useLocation } from "react-router-dom";
import { useIsFetching } from "@tanstack/react-query";
import { Toaster, toast } from "sonner";
import { markRouteStart, markRouteContent, perf } from "./utils/perf";
import { Sidebar } from "./components/Sidebar";
import { BottomNav } from "./components/BottomNav";
import { CommandPalette } from "./components/CommandPalette";
import { ThemeProvider } from "./components/ThemeProvider";
import { useTweaks } from "./state/tweaks";
import { useOnboardingState } from "./api/hooks/onboarding";
import { useOnboardingRedirect } from "./hooks/useOnboardingRedirect";
import ImportProgress from "./components/ImportProgress";
import UnfinishedImportBanner from "./components/UnfinishedImportBanner";
import ShareTargetImport from "./components/ShareTargetImport";
import { useAppBadge } from "./pwa/useAppBadge";
import * as I from "./components/Icons";

const Today = lazy(() => import("./screens/Today"));
const Inbox = lazy(() => import("./screens/Inbox"));
const ImportReview = lazy(() => import("./screens/ImportReview"));
const Accounts = lazy(() => import("./screens/Accounts"));
const AccountTransactions = lazy(() => import("./screens/AccountTransactions"));
const Budget = lazy(() => import("./screens/Budget"));
const Categories = lazy(() => import("./screens/Categories"));
const Recurring = lazy(() => import("./screens/Recurring"));
const Goals = lazy(() => import("./screens/Goals"));
const Journey = lazy(() => import("./screens/Journey"));
const Scenarios = lazy(() => import("./screens/Scenarios"));
const Reports = lazy(() => import("./screens/Reports"));
const PathBack = lazy(() => import("./screens/PathBack"));
const Rules = lazy(() => import("./screens/Rules"));
const Settings = lazy(() => import("./screens/Settings"));
// Server-mode-only admin surface; the route resolves for everyone but the
// screen itself renders nothing outside server mode / for non-admins.
const UsersAdmin = lazy(() => import("./screens/server/UsersAdmin"));
const Copilot = lazy(() => import("./screens/Copilot"));
const CopilotAgUiSpike = lazy(() => import("./screens/CopilotAgUiSpike"));
const Recipes = lazy(() => import("./screens/Recipes"));
// DEV-only: gallery of the Copilot generative-UI blocks (never routed in prod builds).
const GenUiPreview = lazy(() => import("./dev/GenUiPreview"));
import Onboarding from "./screens/Onboarding";

function recoverRoute() {
  window.location.reload();
}

function RouteLoadProblem({
  title,
  message,
  error,
}: {
  title: string;
  message: string;
  error?: unknown;
}) {
  const detail = error instanceof Error ? error.message : typeof error === "string" ? error : null;

  return (
    <section className="stub route-load-problem" role="alert">
      <div className="card">
        <p className="eyebrow">Screen recovery</p>
        <h1>{title}</h1>
        <p className="muted">{message}</p>
        {detail && <pre>{detail}</pre>}
        <button type="button" className="btn primary" onClick={recoverRoute}>
          Reload screen
        </button>
      </div>
    </section>
  );
}

type RouteErrorBoundaryProps = {
  children: ReactNode;
  resetKey: string;
};

class RouteErrorBoundary extends Component<RouteErrorBoundaryProps, { error: unknown | null }> {
  state = { error: null };

  static getDerivedStateFromError(error: unknown) {
    return { error };
  }

  componentDidCatch(error: unknown, info: ErrorInfo) {
    console.error("Route render failed", error, info);
  }

  componentDidUpdate(prevProps: RouteErrorBoundaryProps) {
    if (prevProps.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null });
    }
  }

  render() {
    if (this.state.error) {
      return (
        <RouteLoadProblem
          title="This screen failed to load"
          message="FinSight hit a recoverable screen-loading error. Reload this screen instead of staying on a blank page."
          error={this.state.error}
        />
      );
    }

    return this.props.children;
  }
}

function PageLoader() {
  const [isSlow, setIsSlow] = useState(false);

  useEffect(() => {
    const timer = window.setTimeout(() => setIsSlow(true), 4000);
    return () => window.clearTimeout(timer);
  }, []);

  return (
    <div className="stub route-loader" role="status" aria-label="Loading page">
      <span className="spinner" aria-hidden="true" />
      <p>Loading…</p>
      {isSlow && (
        <div className="route-loader-recovery">
          <p className="muted">
            This is taking longer than expected. In desktop dev mode this can happen after dependency
            changes or stale Vite chunks.
          </p>
          <button type="button" className="btn outline" onClick={recoverRoute}>
            Reload screen
          </button>
        </div>
      )}
    </div>
  );
}

/**
 * Records nav-intent→content-painted per route when perf instrumentation is
 * on.
 *
 * CORRECTNESS NOTE (found via a real driven measurement pass, not assumed):
 * on a route change, React commits the pathname-change effect and re-renders
 * `useIsFetching()` in the SAME pass, but the destination route's OWN queries
 * haven't mounted/started fetching yet — so the very first read of
 * `isFetching` after navigating is the previous route's already-settled value
 * (0), not a signal that the new route is done. Closing on that first `0`
 * made every route report ~0ms regardless of real cost. The fix: only close
 * on a transition to `0` that was preceded by an observed `>0` for THIS
 * route, with a short grace fallback for routes that are genuinely served
 * entirely from a warm (prefetched) cache and never fetch at all.
 */
export function RouteTimer() {
  const { pathname } = useLocation();
  const isFetching = useIsFetching();
  const sawFetchingRef = useRef(false);
  const armedForRef = useRef<string | null>(null);

  useEffect(() => {
    markRouteStart(pathname);
    sawFetchingRef.current = false;
    armedForRef.current = pathname;
    // Grace fallback: if this route never triggers a real fetch (fully
    // served from cache, e.g. prefetch-warmed), close it after one short
    // window rather than leaving it unmeasured or misreading a stale value.
    const t = setTimeout(() => {
      if (armedForRef.current === pathname && !sawFetchingRef.current) {
        markRouteContent(pathname);
        armedForRef.current = null;
      }
    }, 32);
    return () => clearTimeout(t);
  }, [pathname]);

  useEffect(() => {
    if (armedForRef.current !== pathname) return;
    if (isFetching > 0) {
      sawFetchingRef.current = true;
    } else if (sawFetchingRef.current) {
      // A real transition: this route fetched, and has now settled.
      markRouteContent(pathname);
      armedForRef.current = null;
    }
  }, [isFetching, pathname]);

  return null;
}

export function App() {
  const location = useLocation();
  const { data: onboarding } = useOnboardingState();
  useOnboardingRedirect(onboarding);
  const [cmdOpen, setCmdOpen] = useState(false);
  const { privacy, setPrivacy } = useTweaks();
  const isOnboarding = location.pathname === "/onboarding";

  // Installed-PWA icon badge. App-level on purpose: the badge's job is to be
  // right while the user is on some other screen entirely.
  useAppBadge();

  // Global keyboard shortcuts
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const meta = e.metaKey || e.ctrlKey;
      if (meta && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setCmdOpen((o) => !o);
      }
      if (meta && e.key === ".") {
        e.preventDefault();
        setPrivacy(!privacy);
      }
      // Perf-measurement hotkeys (see utils/perf.ts). Runtime-toggleable so a
      // driven measurement pass works on a release build with no devtools and
      // no reload: Ctrl+Alt+P flips instrumentation on/off, Ctrl+Alt+S copies
      // summary() (aggregated stats), Ctrl+Alt+E copies export() (every raw
      // sample) — summary() for a quick read, export() for a full before/after
      // diff.
      if (e.ctrlKey && e.altKey && e.key.toLowerCase() === "p") {
        e.preventDefault();
        const on = perf.toggle();
        toast(on ? "Perf instrumentation ON" : "Perf instrumentation OFF");
      }
      if (e.ctrlKey && e.altKey && e.key.toLowerCase() === "s") {
        e.preventDefault();
        void perf.copySummaryToClipboard().then(
          () => toast("Perf summary copied to clipboard"),
          () => toast.error("Could not copy perf summary")
        );
      }
      if (e.ctrlKey && e.altKey && e.key.toLowerCase() === "e") {
        e.preventDefault();
        void perf.copyExportToClipboard().then(
          () => toast("Perf raw samples copied to clipboard"),
          () => toast.error("Could not copy perf raw samples")
        );
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setPrivacy, privacy]);

  return (
    <ThemeProvider>
      <RouteTimer />
      <a href="#main" className="skip-link">
        Skip to main content
      </a>
      {isOnboarding ? (
        <RouteErrorBoundary resetKey={location.key}>
          <Suspense fallback={<PageLoader />}>
            <Onboarding />
          </Suspense>
        </RouteErrorBoundary>
      ) : (
        <div className="app">
          <Sidebar onOpenCmd={() => setCmdOpen(true)} />
          <main id="main" className="main" tabIndex={-1}>
            <div className="main-inner">
              <UnfinishedImportBanner />
              <ImportProgress />
              {/* Renders nothing unless this launch came from the OS share sheet. */}
              <ShareTargetImport />
              <RouteErrorBoundary resetKey={location.key}>
                <Suspense fallback={<PageLoader />}>
                  <Routes>
                    <Route path="/" element={<Today />} />
                    <Route path="/inbox" element={<Inbox />} />
                    <Route path="/import-review" element={<ImportReview />} />
                    {/* Insights retired — its action items live in Inbox, its agent memory in Settings. */}
                    <Route path="/insights" element={<Navigate to="/inbox" replace />} />
                    <Route path="/accounts" element={<Accounts />} />
                    <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
                    {/* All-accounts ledger — where Inbox review CTAs deep-link (?filter=…). */}
                    <Route path="/transactions" element={<AccountTransactions />} />
                    <Route path="/budget" element={<Budget />} />
                    <Route path="/categories" element={<Categories />} />
                    <Route path="/recurring" element={<Recurring />} />
                    <Route path="/goals" element={<Goals />} />
                    <Route path="/journey" element={<Journey />} />
                    <Route path="/scenarios" element={<Scenarios />} />
                    <Route path="/reports" element={<Reports />} />
                    <Route path="/path-back" element={<PathBack />} />
                    <Route path="/rules" element={<Rules />} />
                    <Route path="/settings" element={<Settings />} />
                    <Route path="/settings/users" element={<UsersAdmin />} />
                    <Route path="/copilot" element={<Copilot />} />
                    <Route path="/copilot/ag-ui-spike" element={<CopilotAgUiSpike />} />
                    <Route path="/recipes" element={<Recipes />} />
                    {import.meta.env.DEV && (
                      <Route path="/dev/genui-preview" element={<GenUiPreview />} />
                    )}
                  </Routes>
                </Suspense>
              </RouteErrorBoundary>
            </div>
          </main>
          <BottomNav />
        </div>
      )}

      {privacy && (
        <button
          className="privacy-badge"
          onClick={() => setPrivacy(false)}
          aria-label="Privacy mode active — click to disable"
          title="Privacy mode · ⌘. to toggle"
        >
          <I.EyeOff width="14" height="14" aria-hidden="true" />
          <span>Privacy mode · ⌘.</span>
        </button>
      )}

      <CommandPalette open={cmdOpen} onClose={() => setCmdOpen(false)} />
      <Toaster richColors position="bottom-right" />
    </ThemeProvider>
  );
}
