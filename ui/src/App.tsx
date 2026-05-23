import { useEffect } from "react";
import { Route, Routes, useLocation, useNavigate } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { ThemeProvider } from "./components/ThemeProvider";
import Today from "./screens/Today";
import Transactions from "./screens/Transactions";
import Accounts from "./screens/Accounts";
import Budget from "./screens/Budget";
import Categories from "./screens/Categories";
import Settings from "./screens/Settings";
import Onboarding from "./screens/Onboarding";
import { useOnboardingState } from "./api/hooks/onboarding";
import ImportProgress from "./components/ImportProgress";
import UnfinishedImportBanner from "./components/UnfinishedImportBanner";

export function App() {
  const { data: onboarding } = useOnboardingState();
  const navigate = useNavigate();
  const location = useLocation();
  useEffect(() => {
    if (!onboarding) return;
    const shouldShow =
      onboarding.account_count === 0 && !onboarding.completion_marked;
    if (shouldShow && location.pathname !== "/onboarding") {
      navigate("/onboarding", { replace: true });
    }
  }, [onboarding, location.pathname, navigate]);

  return (
    <ThemeProvider>
      <div className="app">
        <Sidebar />
        <main className="main">
          <UnfinishedImportBanner />
          <ImportProgress />
          <Routes>
            <Route path="/" element={<Today />} />
            <Route path="/accounts" element={<Accounts />} />
            <Route path="/transactions" element={<Transactions />} />
            <Route path="/budget" element={<Budget />} />
            <Route path="/categories" element={<Categories />} />
            <Route path="/settings" element={<Settings />} />
            <Route path="/onboarding" element={<Onboarding />} />
          </Routes>
        </main>
      </div>
    </ThemeProvider>
  );
}
