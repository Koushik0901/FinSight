import { Suspense, lazy, useEffect, useState } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { Toaster } from "sonner";
import { Sidebar } from "./components/Sidebar";
import { CommandPalette } from "./components/CommandPalette";
import { ThemeProvider } from "./components/ThemeProvider";
import { useTweaks } from "./state/tweaks";
import { useOnboardingState } from "./api/hooks/onboarding";
import ImportProgress from "./components/ImportProgress";
import UnfinishedImportBanner from "./components/UnfinishedImportBanner";
import * as I from "./components/Icons";

const Today = lazy(() => import("./screens/Today"));
const Inbox = lazy(() => import("./screens/Inbox"));
const ImportReview = lazy(() => import("./screens/ImportReview"));
const Insights = lazy(() => import("./screens/Insights"));
const Transactions = lazy(() => import("./screens/Transactions"));
const Accounts = lazy(() => import("./screens/Accounts"));
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
const Recipes = lazy(() => import("./screens/Recipes"));
const Onboarding = lazy(() => import("./screens/Onboarding"));

function PageLoader() {
  return (
    <div className="stub" role="status" aria-label="Loading page">
      <span className="spinner" aria-hidden="true" />
      <p>Loading…</p>
    </div>
  );
}

export function App() {
  const { data: onboarding } = useOnboardingState();
  const navigate = useNavigate();
  const location = useLocation();
  const [cmdOpen, setCmdOpen] = useState(false);
  const { privacy, setPrivacy } = useTweaks();

  // Auto-redirect to onboarding for fresh installs
  useEffect(() => {
    if (!onboarding) return;
    const shouldShow = onboarding.account_count === 0 && !onboarding.completion_marked;
    if (shouldShow && location.pathname !== "/onboarding") {
      navigate("/onboarding", { replace: true });
    }
  }, [onboarding, location.pathname, navigate]);

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
      <div className="app">
        <Sidebar onOpenCmd={() => setCmdOpen(true)} />
        <main id="main" className="main" tabIndex={-1}>
          <div className="main-inner">
            <UnfinishedImportBanner />
            <ImportProgress />
            <Suspense fallback={<PageLoader />}>
              <Routes>
                <Route path="/" element={<Today />} />
                <Route path="/inbox" element={<Inbox />} />
                <Route path="/import-review" element={<ImportReview />} />
                <Route path="/insights" element={<Insights />} />
                <Route path="/accounts" element={<Accounts />} />
                <Route path="/transactions" element={<Transactions />} />
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
                <Route path="/recipes" element={<Recipes />} />
                <Route path="/onboarding" element={<Onboarding />} />
              </Routes>
            </Suspense>
          </div>
        </main>
      </div>

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
