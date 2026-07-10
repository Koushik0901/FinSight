import { useState } from "react";
import { useHouseholdMembers } from "../api/hooks/household";
import { useFinancialMetrics } from "../api/hooks/metrics";
import { money } from "../utils/format";

/**
 * Per-person cashflow & balances on Today. Hidden unless the household has 2+
 * members (with one person, "Everyone" already IS that person). Numbers come
 * from the member-weighted metrics layer — joint accounts split equally — so
 * each person's slice plus the unassigned residual reconciles to the household
 * total. Selecting "Everyone" shows the same household figures as the rest of
 * the screen.
 */
export default function PerPersonCard({ currency }: { currency: string }) {
  const { data: members = [] } = useHouseholdMembers();
  const [memberId, setMemberId] = useState<string | null>(null);
  const { data: m } = useFinancialMetrics(memberId);

  // Nothing to switch between with fewer than two people.
  if (members.length < 2) return null;

  const selected = members.find((x) => x.id === memberId) ?? null;
  const rate = m?.thisMonthSavingsRatePct ?? 0;
  const activeChip = { borderColor: "var(--accent)", color: "var(--accent)" } as const;

  return (
    <section className="section">
      <div className="card">
        <div
          className="row"
          style={{ justifyContent: "space-between", alignItems: "center", gap: 12, flexWrap: "wrap", marginBottom: 14 }}
        >
          <div className="eyebrow"><span className="dot" />Per person</div>
          <div className="row row-sm wrap" role="tablist" aria-label="Filter by household member">
            <button
              type="button"
              className="chip"
              role="tab"
              aria-selected={memberId === null}
              style={memberId === null ? activeChip : undefined}
              onClick={() => setMemberId(null)}
            >
              Everyone
            </button>
            {members.map((mem) => (
              <button
                key={mem.id}
                type="button"
                className="chip"
                role="tab"
                aria-selected={memberId === mem.id}
                style={memberId === mem.id ? activeChip : undefined}
                onClick={() => setMemberId(mem.id)}
              >
                <span
                  className="cswatch"
                  style={{ background: mem.color || "var(--ink-faint)", width: 8, height: 8, marginRight: 6 }}
                />
                {mem.name}
              </button>
            ))}
          </div>
        </div>
        <div className="stat-row">
          <div className="stat">
            <div className="label">Income (this month)</div>
            <div className="value money">{money(m?.thisMonthIncomeCents ?? 0, { currency })}</div>
          </div>
          <div className="stat">
            <div className="label">Spending (this month)</div>
            <div className="value money">{money(m?.thisMonthExpenseCents ?? 0, { currency })}</div>
          </div>
          <div className="stat">
            <div className="label">Savings rate</div>
            <div className="value" style={{ color: rate >= 0 ? "var(--accent)" : "var(--negative)" }}>{rate}%</div>
          </div>
          <div className="stat">
            <div className="label">Liquid balance</div>
            <div className="value money">{money(m?.liquidCents ?? 0, { currency })}</div>
          </div>
        </div>
        <div className="muted" style={{ fontSize: 12, marginTop: 10 }}>
          {selected
            ? `${selected.name}'s share — joint accounts are split equally; household-shared accounts aren't counted here.`
            : "Whole household. Pick a person to see their share, with joint accounts split equally."}
        </div>
      </div>
    </section>
  );
}
