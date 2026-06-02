import { useEffect, useState } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { Toaster } from "sonner";
import { Sidebar } from "./components/Sidebar";
import { CommandPalette } from "./components/CommandPalette";
import { ThemeProvider } from "./components/ThemeProvider";
import { useTweaks } from "./state/tweaks";
import Today from "./screens/Today";
import Insights from "./screens/Insights";
import Transactions from "./screens/Transactions";
import Accounts from "./screens/Accounts";
import Budget from "./screens/Budget";
import Categories from "./screens/Categories";
import Recurring from "./screens/Recurring";
import Goals from "./screens/Goals";
import Reports from "./screens/Reports";
import Rules from "./screens/Rules";
import Settings from "./screens/Settings";
import Onboarding from "./screens/Onboarding";
import { useOnboardingState } from "./api/hooks/onboarding";
import ImportProgress from "./components/ImportProgress";
import UnfinishedImportBanner from "./components/UnfinishedImportBanner";
import * as I from "./components/Icons";

export function App() {
  const { data: onboarding } = useOnboardingState();
  const navigate = useNavigate();
  const location = useLocation();
  const [cmdOpen, setCmdOpen] = useState(false);
  const { privacy, setPrivacy } = useTweaks();

  // Auto-redirect to onboarding for fresh installs
  useEffect(() => {
    if (!onboarding) return;
    const shouldShow =
      onboarding.account_count === 0 && !onboarding.completion_marked;
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
  }, [privacy, setPrivacy]);

  return (
    <ThemeProvider>
      <div className="app">
        <Sidebar onOpenCmd={() => setCmdOpen(true)} />
        <main className="main">
          <div className="main-inner">
            <UnfinishedImportBanner />
            <ImportProgress />
            <Routes>
              <Route path="/"             element={<Today />} />
              <Route path="/insights"     element={<Insights />} />
              <Route path="/accounts"     element={<Accounts />} />
              <Route path="/transactions" element={<Transactions />} />
              <Route path="/budget"       element={<Budget />} />
              <Route path="/categories"   element={<Categories />} />
              <Route path="/recurring"    element={<Recurring />} />
              <Route path="/goals"        element={<Goals />} />
              <Route path="/reports"      element={<Reports />} />
              <Route path="/rules"        element={<Rules />} />
              <Route path="/settings"     element={<Settings />} />
              <Route path="/onboarding"   element={<Onboarding />} />
            </Routes>
          </div>
        </main>
      </div>

      {/* Privacy mode badge */}
      {privacy && (
        <button
          className="privacy-badge"
          onClick={() => setPrivacy(false)}
          aria-label="Privacy mode active — click to disable"
          title="Privacy mode · ⌘. to toggle"
        >
          <I.EyeOff width="14" height="14" />
          <span>Privacy mode · ⌘.</span>
        </button>
      )}

      <CommandPalette open={cmdOpen} onClose={() => setCmdOpen(false)} />
      <Toaster richColors position="bottom-right" />
    </ThemeProvider>
  );
}
