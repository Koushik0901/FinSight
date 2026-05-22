import { useTransactions } from "../api/hooks/transactions";

function formatMoney(cents: number) {
  const sign = cents < 0 ? "-" : "";
  return `${sign}$${(Math.abs(cents) / 100).toFixed(2)}`;
}

function formatDate(iso: string) {
  return new Date(iso).toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export default function Transactions() {
  const { data, isLoading, error } = useTransactions();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;
  if (!data || data.length === 0) return <div className="stub">No transactions yet.</div>;

  return (
    <section>
      <h1 style={{ fontSize: 32, fontWeight: 600, marginTop: 0, marginBottom: 24 }}>Transactions</h1>
      <table style={{ width: "100%", borderCollapse: "collapse" }}>
        <thead>
          <tr style={{ textAlign: "left", color: "var(--text-3)", fontSize: 11, letterSpacing: "0.06em", textTransform: "uppercase" }}>
            <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Date</th>
            <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Merchant</th>
            <th scope="col" style={{ padding: "8px 0", fontWeight: 500 }}>Category</th>
            <th scope="col" style={{ padding: "8px 0", fontWeight: 500, textAlign: "right" }}>Amount</th>
          </tr>
        </thead>
        <tbody>
          {data.map((t) => (
            <tr key={t.id} style={{ borderTop: "1px solid var(--hairline)" }}>
              <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>{formatDate(t.posted_at)}</td>
              <td style={{ padding: "12px 0", display: "flex", alignItems: "center", gap: 12 }}>
                <span
                  aria-label={`${t.merchant_label ?? t.merchant_raw} merchant tile`}
                  style={{
                    width: 28,
                    height: 28,
                    borderRadius: 6,
                    background: t.merchant_color ?? "var(--surface-2)",
                    color: "var(--accent-ink)",
                    fontSize: 11,
                    fontWeight: 600,
                    display: "grid",
                    placeItems: "center",
                  }}
                >
                  {t.merchant_initials ?? "?"}
                </span>
                <span>{t.merchant_label ?? t.merchant_raw}</span>
              </td>
              <td style={{ padding: "12px 0", color: "var(--text-2)", fontSize: 13 }}>
                {t.category_label ?? "Uncategorized"}
              </td>
              <td style={{ padding: "12px 0", textAlign: "right", fontFamily: "Geist Mono, monospace" }}>
                <span className="money">{formatMoney(t.amount_cents)}</span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
