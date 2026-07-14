import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { commands, type AccountSummary } from "../api/client";
import { useAccountPositions, useInvestmentSummary } from "../api/hooks/investments";
import { invalidateDomains } from "../api/invalidation";
import { money } from "../utils/format";
import Button from "./Button";

/** Positions + portfolio estimate for an Investment account, derived from
 *  imported trade rows. Valuation uses the LAST TRADE PRICE from the import —
 *  a stale estimate, so the account balance is only written via the explicit
 *  "Set balance from estimate" action, never silently. */
export default function HoldingsCard({ account }: { account: AccountSummary }) {
  const qc = useQueryClient();
  const { data: positions = [] } = useAccountPositions(account.id);
  const { data: summary } = useInvestmentSummary(account.id);

  const currency = account.currency || "USD";
  const fmt = (cents: number) => money(cents, { currency, decimals: 2 });

  // Nothing imported yet → no card. (Interest-only or cash-only accounts with
  // a summary still get the totals row.)
  if (!summary || (positions.length === 0 && summary.dividendIncomeCents === 0 && summary.interestIncomeCents === 0 && summary.cashCents === 0)) {
    return null;
  }

  const setBalanceFromEstimate = async () => {
    const r = await commands.setAccountBalance(account.id, summary.portfolioEstimateCents);
    if (r.status === "error") {
      toast.error("Could not set balance", { description: r.error.message });
      return;
    }
    toast.success(`Balance set to ${fmt(summary.portfolioEstimateCents)}`, {
      description: "From cash + positions at the last imported trade price.",
    });
    invalidateDomains(qc, "accounts");
  };

  return (
    <div className="section" data-testid="holdings-card">
      <div className="card">
        <div className="row wrap" style={{ justifyContent: "space-between", alignItems: "flex-start" }}>
          <div>
            <div className="eyebrow"><span className="dot" />Holdings</div>
            <div className="row row-sm wrap" style={{ marginTop: 8 }}>
              <span className="chip">Dividends {fmt(summary.dividendIncomeCents)}</span>
              <span className="chip">Interest {fmt(summary.interestIncomeCents)}</span>
              {summary.withholdingTaxCents !== 0 && (
                <span className="chip">Withholding tax {fmt(summary.withholdingTaxCents)}</span>
              )}
              {summary.hasNegativeQuantity && (
                <span className="chip warning">
                  Positions may be incomplete — import the full history
                </span>
              )}
            </div>
          </div>
          <div style={{ textAlign: "right" }}>
            <div className="muted" style={{ fontSize: 12 }}>Portfolio estimate</div>
            <div className="figure money" style={{ fontSize: 22 }}>
              ≈ {fmt(summary.portfolioEstimateCents)}
            </div>
            <div className="muted" style={{ fontSize: 12 }}>
              Cash {fmt(summary.cashCents)} + positions at last trade price
            </div>
            <Button
              variant="outline"
              size="sm"
              style={{ marginTop: 6 }}
              onClick={setBalanceFromEstimate}
            >
              Set balance from estimate
            </Button>
          </div>
        </div>

        {positions.length > 0 && (
          <table className="tbl" style={{ marginTop: 12 }}>
            <thead>
              <tr>
                <th>Symbol</th>
                <th>Name</th>
                <th className="right">Quantity</th>
                <th className="right">Last price</th>
                <th className="right">Market value</th>
              </tr>
            </thead>
            <tbody>
              {positions.map((p) => (
                <tr key={p.symbol}>
                  <td><span className="mono">{p.symbol}</span></td>
                  <td className="muted">{p.name ?? "—"}</td>
                  <td className="right"><span className="figure">{p.quantity.toFixed(4)}</span></td>
                  <td className="right">
                    <span className="figure money">
                      {p.lastPrice != null ? money(Math.round(p.lastPrice * 100), { currency, decimals: 2 }) : "—"}
                    </span>
                  </td>
                  <td className="right">
                    <span className="figure money">
                      {p.marketValueCents != null ? fmt(p.marketValueCents) : "—"}
                    </span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}
