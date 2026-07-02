import {
  Component,
  Suspense,
  lazy,
  useEffect,
  useState,
  type ErrorInfo,
  type ReactNode,
} from "react";
import { Route, Routes, useLocation } from "react-router-dom";
import { Toaster } from "sonner";
import { Sidebar } from "./components/Sidebar";
import { CommandPalette } from "./components/CommandPalette";
import { ThemeProvider } from "./components/ThemeProvider";
import { useTweaks } from "./state/tweaks";
import { useOnboardingState } from "./api/hooks/onboarding";
import { useOnboardingRedirect } from "./hooks/useOnboardingRedirect";
import ImportProgress from "./components/ImportProgress";
import UnfinishedImportBanner from "./components/UnfinishedImportBanner";
import * as I from "./components/Icons";

const Today = lazy(() => import("./screens/Today"));
const Inbox = lazy(() => import("./screens/Inbox"));
const ImportReview = lazy(() => import("./screens/ImportReview"));
const Insights = lazy(() => import("./screens/Insights"));
const Accounts = lazy(() => import("./screens/Accounts"));
const AccountTransactions = lazy(() => import("./screens/AccountTransactions"));
const Budget = lazy(() => import("./screens/Budget"));
const Categories = lazy(() => import("./screens/Categories"));
const Recurring = lazy(() => import("./screens/Recurring"));
const Goals = lazy(() => import("./screens/Goals"));
const Journey = lazy(() => import("./screens/Journey"));
const Scenarios = lazy(() => import("./screens/Scenarios"));
const Reports = lazy(() => import("./screens/Reports"));
const Rules = lazy(() => import("./screens/Rules"));
const Settings = lazy(() => import("./screens/Settings"));
const Copilot = lazy(() => import("./screens/Copilot"));
const CopilotAgUiSpike = lazy(() => import("./screens/CopilotAgUiSpike"));
const Recipes = lazy(() => import("./screens/Recipes"));
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

export function App() {
  const location = useLocation();
  const { data: onboarding } = useOnboardingState();
  useOnboardingRedirect(onboarding);
  const [cmdOpen, setCmdOpen] = useState(false);
  const { privacy, setPrivacy } = useTweaks();
  const isOnboarding = location.pathname === "/onboarding";

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
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [setPrivacy, privacy]);

  return (
    <ThemeProvider>
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
              <RouteErrorBoundary resetKey={location.key}>
                <Suspense fallback={<PageLoader />}>
                  <Routes>
                    <Route path="/" element={<Today />} />
                    <Route path="/inbox" element={<Inbox />} />
                    <Route path="/import-review" element={<ImportReview />} />
                    <Route path="/insights" element={<Insights />} />
                    <Route path="/accounts" element={<Accounts />} />
                    <Route path="/accounts/:id/transactions" element={<AccountTransactions />} />
                    <Route path="/budget" element={<Budget />} />
                    <Route path="/categories" element={<Categories />} />
                    <Route path="/recurring" element={<Recurring />} />
                    <Route path="/goals" element={<Goals />} />
                    <Route path="/journey" element={<Journey />} />
                    <Route path="/scenarios" element={<Scenarios />} />
                    <Route path="/reports" element={<Reports />} />
                    <Route path="/rules" element={<Rules />} />
                    <Route path="/settings" element={<Settings />} />
                    <Route path="/copilot" element={<Copilot />} />
                    <Route path="/copilot/ag-ui-spike" element={<CopilotAgUiSpike />} />
                    <Route path="/recipes" element={<Recipes />} />
                  </Routes>
                </Suspense>
              </RouteErrorBoundary>
            </div>
          </main>
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
