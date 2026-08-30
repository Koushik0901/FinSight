import { useMemo, useState } from "react";
import { useDrawerSeed } from "../api/hooks/useDrawerSeed";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useAccounts } from "../api/hooks/accounts";
import { useUpdateGoalMonthly, useUpdateGoalPurpose, useUpdateGoalPriority, useGoalContributions, useContributeToGoal } from "../api/hooks/budget";
import type { GoalDto } from "../api/openapiClient";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";
import { formatCalendarDate } from "../utils/date";

const TYPE_LABELS: Record<string, string> = {
  "save-by-date": "Save by date",
  "build-balance": "Build balance",
  "debt-payoff": "Pay off debt",
  "spending-cap": "Spending cap",
  "sinking-fund": "Sinking fund",
  "needed-for-spending": "Needed for spending",
};

interface Props {
  open: boolean;
  onClose: () => void;
  goal: GoalDto | null;
}

export default function GoalDrawer({ open, onClose, goal }: Props) {
  const updateMonthly = useUpdateGoalMonthly();
  const updatePurpose = useUpdateGoalPurpose();
  const { data: accounts = [] } = useAccounts();
  const updatePriority = useUpdateGoalPriority();
  const [monthly, setMonthly] = useState("");
  const [period, setPeriod] = useState("monthly");
  const [purpose, setPurpose] = useState("");
  const [priority, setPriority] = useState("normal");
  const [strictness, setStrictness] = useState("target");
  const [contribAmount, setContribAmount] = useState("");
  const [contribNote, setContribNote] = useState("");
  // The contribution ledger only applies to manual goals; account-linked goals
  // derive their balance from the account.
  const isManual = !!goal && !goal.accountId;
  const contribute = useContributeToGoal();
  const { data: contributions = [] } = useGoalContributions(isManual ? goal?.id : undefined);

  const addContribution = async (sign: 1 | -1) => {
    if (!goal) return;
    const dollars = Number(contribAmount);
    if (!Number.isFinite(dollars) || dollars <= 0) {
      toast.error("Enter an amount to record");
      return;
    }
    try {
      await contribute.mutateAsync({
        id: goal.id,
        amountCents: sign * Math.round(dollars * 100),
        note: contribNote.trim() || null,
      });
      setContribAmount("");
      setContribNote("");
      toast.success(sign > 0 ? "Contribution added" : "Withdrawal recorded");
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Could not update the balance");
    }
  };

  const contributionLabel = (c: { note?: string | null | undefined; source: string }) =>
    c.note || (c.source === "opening" ? "Opening balance" : c.source === "sweep" ? "Parked surplus" : "Contribution");

  useDrawerSeed(open, goal?.id, () => {
    if (!goal) {
      setMonthly("");
      setPeriod("monthly");
      setPurpose("");
      return;
    }
    setMonthly(String((goal.monthlyCents / 100).toFixed(2)));
    setPeriod((goal as unknown as { period?: string }).period || "monthly");
    setPurpose(goal.purpose ?? "");
    setPriority(goal.priority || "normal");
    setStrictness(goal.deadlineStrictness || "target");
  });
  const linkedAccount = useMemo(
    () => accounts.find((account) => account.id === goal?.accountId) ?? null,
    [accounts, goal?.accountId]
  );

  async function save() {
    if (!goal) return;
    const nextMonthly = Number(monthly);
    const nextPurpose = purpose.trim();
    const currentMonthly = goal.monthlyCents / 100;
    const currentPurpose = goal.purpose ?? "";
    const currentPeriod = (goal as unknown as { period?: string }).period || "monthly";

    try {
      const tasks: Promise<unknown>[] = [];
      const monthlyChanged = Number.isFinite(nextMonthly) && nextMonthly !== currentMonthly;
      const periodChanged = period !== currentPeriod;
      if (monthlyChanged || periodChanged) {
        const cents = Number.isFinite(nextMonthly) ? Math.round(nextMonthly * 100) : goal.monthlyCents;
        tasks.push(updateMonthly.mutateAsync({ id: goal.id, monthlyCents: cents, period }));
      }
      if (nextPurpose !== currentPurpose) {
        tasks.push(updatePurpose.mutateAsync({ id: goal.id, purpose: nextPurpose || null }));
      }
      // Sent as a pair, and only when something actually changed — the planner
      // reads them together, so a partial write would leave an incoherent
      // combination behind.
      const nextStrictness = goal.targetDate ? strictness : "none";
      if (priority !== goal.priority || nextStrictness !== goal.deadlineStrictness) {
        tasks.push(
          updatePriority.mutateAsync({
            id: goal.id,
            priority,
            deadlineStrictness: nextStrictness,
          }),
        );
      }
      if (tasks.length === 0) {
        onClose();
        return;
      }
      await Promise.all(tasks);
      toast.success("Goal updated");
      onClose();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Could not save goal");
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title={goal ? `Edit goal · ${goal.name}` : "Edit goal"}>
      {!goal ? (
        <div className="stub">Select a goal to edit it.</div>
      ) : (
        <div className="drawer-form">
          <div className="card tight" style={{ padding: 16, background: "var(--surface-2)" }}>
            <div className="row row-sm wrap" style={{ marginBottom: 10 }}>
              <span className="chip">{TYPE_LABELS[goal.goalType] || goal.goalType}</span>
              {goal.goalType === "needed-for-spending" && <span className="chip accent">Refills</span>}
              {goal.targetDate && <span className="chip">Target {formatCalendarDate(goal.targetDate, { month: "short", year: "numeric" })}</span>}
            </div>
            <div className="muted">{goal.targetCents > 0 ? `Goal size ${money(goal.targetCents)}` : "No target amount"}</div>
            <div className="muted" style={{ marginTop: 4 }}>{goal.currentCents > 0 ? `Current balance ${money(goal.currentCents)}` : "No current balance recorded"}</div>
            {linkedAccount && <div className="muted" style={{ marginTop: 4 }}>Linked account: {getAccountDisplayName(linkedAccount)}</div>}
            {goal.goalType === "needed-for-spending" && (
              <div className="muted" style={{ marginTop: 8, fontSize: 12 }}>Needed for spending — tracks funded (contributions + balance) minus spent in period. After the due date it refills for the next period (default every 6 months). Available caps at target.</div>
            )}
          </div>

          {isManual && (
            <div className="card tight" style={{ padding: 16 }}>
              <div className="eyebrow" style={{ marginBottom: 10 }}>Balance ledger</div>
              <div className="row row-sm wrap" style={{ alignItems: "flex-end" }}>
                <label style={{ flex: "1 1 110px", margin: 0 }}>
                  Amount ($)
                  <input type="number" min="0" step="0.01" value={contribAmount} onChange={(e) => setContribAmount(e.target.value)} aria-label="Contribution amount" />
                </label>
                <label style={{ flex: "2 1 150px", margin: 0 }}>
                  Note
                  <input type="text" value={contribNote} onChange={(e) => setContribNote(e.target.value)} placeholder="optional" />
                </label>
              </div>
              <div className="row row-sm" style={{ marginTop: 10 }}>
                <button type="button" className="primary" disabled={contribute.isPending} onClick={() => void addContribution(1)}>Add funds</button>
                <button type="button" disabled={contribute.isPending} onClick={() => void addContribution(-1)}>Withdraw</button>
              </div>
              {contributions.length > 0 && (
                <div style={{ marginTop: 14 }}>
                  {contributions.map((c) => (
                    <div key={c.id} className="row" style={{ justifyContent: "space-between", alignItems: "center", padding: "6px 0", borderBottom: "1px solid var(--hairline)" }}>
                      <div style={{ minWidth: 0 }}>
                        <div style={{ fontSize: 13, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{contributionLabel(c)}</div>
                        <div className="muted" style={{ fontSize: 11 }}>{new Date(c.createdAt).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</div>
                      </div>
                      <span className={`money ${c.amountCents >= 0 ? "pos" : "neg"}`}>{c.amountCents >= 0 ? "+" : "−"}{money(Math.abs(c.amountCents))}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          <label>
            Contribution ($)
            <input type="number" step="0.01" value={monthly} onChange={(e) => setMonthly(e.target.value)} />
          </label>

          <label>
            Cadence
            <select value={period} onChange={(e) => setPeriod(e.target.value)}>
              <option value="monthly">Monthly</option>
              <option value="weekly">Weekly</option>
              <option value="biweekly">Biweekly</option>
            </select>
          </label>
          {/* Weekly/biweekly contributions are converted to monthly equivalent (weekly ×52/12, biweekly ×26/12) for ETAs and budgeting. */}
          {period !== "monthly" && monthly && Number(monthly) > 0 && (
            <p className="muted" style={{ fontSize: 12, marginTop: -8 }}>
              ≈ {money(Math.round(Number(monthly) * 100 * (period === "weekly" ? 52 / 12 : 26 / 12)))}/mo
            </p>
          )}

          <label>
            Priority
            <select value={priority} onChange={(e) => setPriority(e.target.value)}>
              <option value="critical">Must fund first</option>
              <option value="high">Important</option>
              <option value="normal">Normal</option>
              <option value="someday">Nice to have</option>
            </select>
          </label>
          {/* Outside the label on purpose: text inside one becomes part of the
              field's accessible name, so a screen reader would announce the
              whole explanation as the control's label. */}
          <p className="muted" style={{ fontSize: 12, marginTop: -8 }}>
            Used when goals compete for the same money. Separate from the order
            the cards are arranged in.
          </p>

          {/* Only offered with a date: an open-ended goal has no deadline to be
              strict about, and the choice would imply it does. */}
          {goal?.targetDate && (
            <label>
              Is {goal.targetDate} firm?
              <select value={strictness} onChange={(e) => setStrictness(e.target.value)}>
                <option value="hard">Fixed — the date can&rsquo;t move</option>
                <option value="target">A target I&rsquo;m aiming for</option>
                <option value="none">No real deadline</option>
              </select>
            </label>
          )}

          <label>
            Purpose
            <textarea rows={4} value={purpose} onChange={(e) => setPurpose(e.target.value)} />
          </label>

          <div className="form-actions">
            <button type="button" onClick={onClose}>Cancel</button>
            <button type="button" className="primary" onClick={() => void save()} disabled={updateMonthly.isPending || updatePurpose.isPending || updatePriority.isPending}>
              Save goal
            </button>
          </div>
        </div>
      )}
    </Drawer>
  );
}
