import { Route, Routes } from "react-router-dom";
import { Sidebar } from "./components/Sidebar";
import { ThemeProvider } from "./components/ThemeProvider";
import Today from "./screens/Today";
import Transactions from "./screens/Transactions";
import Accounts from "./screens/Accounts";
import Budget from "./screens/Budget";
import Categories from "./screens/Categories";
import Settings from "./screens/Settings";
import Onboarding from "./screens/Onboarding";

export function App() {
  return (
    <ThemeProvider>
      <div className="app">
        <Sidebar />
        <main className="main">
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
