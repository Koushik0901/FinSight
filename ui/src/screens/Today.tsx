import { useAccounts } from "../api/hooks/accounts";

function formatMoney(cents: number, currency = "USD") {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

export default function Today() {
  const { data, isLoading, error } = useAccounts();

  if (isLoading) return <div className="stub">Loading…</div>;
  if (error) return <div className="stub">Error: {(error as Error).message}</div>;
  if (!data || data.length === 0) return <div className="stub">No accounts yet.</div>;

  const primary = data[0]!;

  return (
    <section>
      <header>
        <p style={{ color: "var(--text-3)", fontSize: 12, letterSpacing: "0.06em", textTransform: "uppercase" }}>
          Today
        </p>
        <h1 style={{ fontSize: 72, fontWeight: 600, letterSpacing: "-0.02em", margin: "8px 0" }}>
          <span className="money">{formatMoney(primary.balance_cents, primary.currency)}</span>
        </h1>
        <p style={{ color: "var(--text-2)" }}>
          in <strong>{primary.name}</strong> · {primary.bank}
        </p>
      </header>
    </section>
  );
}
