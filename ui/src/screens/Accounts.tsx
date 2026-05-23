import { useState } from "react";
import { useAccounts } from "../api/hooks/accounts";
import AccountDrawer from "../components/AccountDrawer";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

export default function Accounts() {
  const [drawerOpen, setDrawerOpen] = useState(false);
  const { data, isLoading, error } = useAccounts();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;

  return (
    <div className="screen-accounts">
      <header className="screen-header" style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: 24 }}>
        <h1 style={{ fontSize: 32, fontWeight: 600, margin: 0 }}>Accounts</h1>
        <button className="primary" onClick={() => setDrawerOpen(true)}>+ Add account</button>
      </header>

      {(!data || data.length === 0) ? (
        <div className="stub">No accounts yet.</div>
      ) : (
        <table style={{ width: "100%", borderCollapse: "collapse" }}>
          <thead>
            <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Bank</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Name</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Type</th>
              <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Balance</th>
            </tr>
          </thead>
          <tbody>
            {data.map((a) => (
              <tr key={a.id} style={{ borderTop: "1px solid var(--hairline)" }}>
                <td style={{ padding: "12px 0" }}>{a.bank}</td>
                <td style={{ padding: "12px 0" }}>{a.name}</td>
                <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{a.type}</td>
                <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                  <span className="money">{formatMoney(a.balance_cents)}</span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      <AccountDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </div>
  );
}
