import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useAccounts } from "../api/hooks/accounts";
import { useLiabilities } from "../api/hooks/assets";
import { useUpdateGoalMonthly, useUpdateGoalPurpose } from "../api/hooks/budget";
import type { GoalDto } from "../api/client";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";

interface Props {
  open: boolean;
  onClose: () => void;
  goal: GoalDto | null;
}

export default function GoalDrawer({ open, onClose, goal }: Props) {
  const updateMonthly = useUpdateGoalMonthly();
  const updatePurpose = useUpdateGoalPurpose();
  const { data: liabilities = [] } = useLiabilities();
  const { data: accounts = [] } = useAccounts();
  const [monthly, setMonthly] = useState("");
  const [purpose, setPurpose] = useState("");

  useEffect(() => {
    if (!goal) {
      setMonthly("");
      setPurpose("");
      return;
    }
    setMonthly(String((goal.monthlyCents / 100).toFixed(2)));
    setPurpose(goal.purpose ?? "");
  }, [goal?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  const linkedLiability = useMemo(
    () => liabilities.find((liability) => liability.id === goal?.liabilityId) ?? null,
    [goal?.liabilityId, liabilities]
  );
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

    try {
      const tasks: Promise<unknown>[] = [];
      if (Number.isFinite(nextMonthly) && nextMonthly !== currentMonthly) {
        tasks.push(updateMonthly.mutateAsync({ id: goal.id, monthlyCents: Math.round(nextMonthly * 100) }));
      }
      if (nextPurpose !== currentPurpose) {
        tasks.push(updatePurpose.mutateAsync({ id: goal.id, purpose: nextPurpose || null }));
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
              <span className="chip">{goal.goalType}</span>
              {goal.targetDate && <span className="chip">Target {new Date(goal.targetDate).toLocaleDateString("en-US", { month: "short", year: "numeric" })}</span>}
            </div>
            <div className="muted">{goal.targetCents > 0 ? `Goal size ${money(goal.targetCents, { currency: "USD" })}` : "No target amount"}</div>
            <div className="muted" style={{ marginTop: 4 }}>{goal.currentCents > 0 ? `Current balance ${money(goal.currentCents, { currency: "USD" })}` : "No current balance recorded"}</div>
            {linkedLiability && <div className="muted" style={{ marginTop: 4 }}>Linked liability: {linkedLiability.name}</div>}
            {linkedAccount && <div className="muted" style={{ marginTop: 4 }}>Linked account: {getAccountDisplayName(linkedAccount)}</div>}
          </div>

          <label>
            Monthly contribution ($)
            <input type="number" step="0.01" value={monthly} onChange={(e) => setMonthly(e.target.value)} />
          </label>

          <label>
            Purpose
            <textarea rows={4} value={purpose} onChange={(e) => setPurpose(e.target.value)} />
          </label>

          <div className="form-actions">
            <button type="button" onClick={onClose}>Cancel</button>
            <button type="button" className="primary" onClick={() => void save()} disabled={updateMonthly.isPending || updatePurpose.isPending}>
              Save goal
            </button>
          </div>
        </div>
      )}
    </Drawer>
  );
}
