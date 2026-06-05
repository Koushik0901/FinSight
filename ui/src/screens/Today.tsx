import { useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import { useAccounts } from "../api/hooks/accounts";
import { useNeedsReviewCount } from "../api/hooks/agent";
import { useCategoriesWithSpending } from "../api/hooks/transactions";
import { commands, type MonthTotals, type AccountSummary } from "../api/client";
import AgentActivityFeed from "../components/AgentActivityFeed";
import { useNetWorth } from "../api/hooks/networth";

function fmt(cents: number, currency = "USD") {
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    maximumFractionDigits: 0,
  }).format(cents / 100);
}

function useMonthTotals() {
  return useQuery<MonthTotals>({
    queryKey: ["month-totals"],
    queryFn: async () => {
      const result = await commands.getMonthTotals();
      if (result.status === "error") throw new Error(result.error.message);
      return result.data;
    },
    refetchInterval: 60_000,
  });
}

function AccountDot({ account }: { account: AccountSummary }) {
  return (
    <div style={{
      display: "flex",
      alignItems: "center",
      justifyContent: "space-between",
      padding: "10px 0",
      borderBottom: "1px solid var(--hairline)",
    }}>
      <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
        <span style={{
          width: 8,
          height: 8,
          borderRadius: 999,
          background: account.color || "var(--ink-faint)",
          boxShadow: `0 0 6px ${account.color || "var(--ink-faint)"}`,
          display: "inline-block",
          flexShrink: 0,
        }} />
        <div>
          <div style={{ fontSize: 14 }}>{account.name}</div>
          <div className="muted" style={{ fontSize: 12 }}>{account.bank}</div>
        </div>
      </div>
      <div className="num tabular money" style={{ fontSize: 14, fontWeight: 500 }}>
        {fmt(account.balance_cents, account.currency)}
      </div>
    </div>
  );
}

export default function Today() {
  const navigate = useNavigate();
  const { data: accounts = [], isLoading: accLoading } = useAccounts();
  const { data: totals, isLoading: totLoading } = useMonthTotals();
  const { data: cats = [] } = useCategoriesWithSpending();
  const { data: needsReview = 0 } = useNeedsReviewCount();
  const netWorth = useNetWorth();

  const now = new Date();
  const dateLabel = now.toLocaleDateString("en-US", { weekday: "long", month: "long", day: "numeric" });
  const monthLabel = now.toLocaleString("default", { month: "long" });

  const primaryCurrency = accounts[0]?.currency ?? "USD";

  const isLoading = accLoading || totLoading;

  if (isLoading) return <div className="stub">Loading…</div>;
  if (accounts.length === 0) return <div className="stub">No accounts yet.</div>;

  const activeCats = cats.filter((c) => c.thisMonthCents > 0);
  const totalSpend = activeCats.reduce((s, c) => s + c.thisMonthCents, 0) || 1;

  return (
    <div className="screen">
      {/* Date header */}
      <div style={{ marginBottom: 24 }}>
        <div className="eyebrow" style={{ marginBottom: 8 }}>
          <span className="dot" />
          {dateLabel}
        </div>

        {/* Net worth hero */}
        <div style={{ display: "flex", alignItems: "baseline", gap: 16, flexWrap: "wrap" }}>
          <div className="figure money" style={{
            fontSize: 64,
            lineHeight: 1,
            letterSpacing: "-0.035em",
            color: netWorth >= 0 ? "var(--ink)" : "var(--negative)",
          }}>
            {fmt(netWorth, primaryCurrency)}
          </div>
          <div className="muted" style={{ fontSize: 16 }}>
            net worth · {accounts.length} account{accounts.length !== 1 ? "s" : ""} + assets − liabilities
          </div>
        </div>
      </div>

      {/* 4-stat row */}
      {totals && (
        <div className="stat-row">
          <div className="stat">
            <div className="label">{monthLabel} income</div>
            <div className="value figure money num pos">{fmt(totals.incomeCents)}</div>
            <div className="sub muted">{totals.txnCount} transactions</div>
          </div>
          <div className="stat">
            <div className="label">{monthLabel} expenses</div>
            <div className="value figure money">{fmt(totals.expenseCents)}</div>
            <div className="sub muted">
              {fmt(totals.netCents) + (totals.netCents >= 0 ? " saved" : " deficit")}
            </div>
          </div>
          <div className={`stat${totals.savingsRatePct > 0 ? " accent" : ""}`}>
            <div className="label">Savings rate</div>
            <div className={`value figure num${totals.savingsRatePct > 0 ? "" : " neg"}`}>
              {totals.savingsRatePct}%
            </div>
            <div className="sub muted">of income kept</div>
          </div>
          <div className="stat">
            <div className="label">Accounts</div>
            <div className="value">{accounts.length}</div>
            <div className="sub muted">
              {accounts.length} active
            </div>
          </div>
        </div>
      )}

      {/* Category stream bar */}
      {activeCats.length > 0 && (
        <div style={{ marginTop: 20 }}>
          <div className="eyebrow" style={{ marginBottom: 8 }}>Spending this month</div>
          <div className="stream" style={{ height: 14, borderRadius: 8 }}>
            {activeCats.map((c) => (
              <span
                key={c.id}
                title={`${c.label}: ${fmt(c.thisMonthCents)}`}
                style={{
                  width: `${(c.thisMonthCents / totalSpend) * 100}%`,
                  background: c.color || "var(--ink-faint)",
                }}
              />
            ))}
          </div>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "6px 16px", marginTop: 8 }}>
            {activeCats.slice(0, 6).map((c) => (
              <span key={c.id} style={{ display: "flex", alignItems: "center", gap: 6, fontSize: 12, color: "var(--ink-mute)" }}>
                <span style={{ width: 8, height: 8, borderRadius: 2, background: c.color || "var(--ink-faint)", display: "inline-block" }} />
                {c.label}
                <span className="tabular" style={{ color: "var(--ink-faint)", fontFamily: "var(--mono)", fontSize: 11 }}>
                  {fmt(c.thisMonthCents)}
                </span>
              </span>
            ))}
          </div>
        </div>
      )}

      {/* Agent feed + needs-review */}
      <div style={{ marginTop: 24 }}>
        <AgentActivityFeed />

        {needsReview > 0 && (
          <button
            onClick={() => navigate("/transactions")}
            className="chip warning"
            style={{ marginTop: 12, cursor: "pointer", border: "none", padding: "8px 14px", fontSize: 13 }}
            aria-label={`${needsReview} transactions need review`}
          >
            ⚠ {needsReview} transaction{needsReview === 1 ? "" : "s"} need{needsReview === 1 ? "s" : ""} review →
          </button>
        )}
      </div>

      {/* Account list */}
      {accounts.length > 1 && (
        <div className="section">
          <div className="card tight">
            <div className="card-head" style={{ padding: "12px 18px" }}>
              <div className="eyebrow">All accounts</div>
            </div>
            <div style={{ padding: "0 18px" }}>
              {accounts.map((a) => <AccountDot key={a.id} account={a} />)}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
