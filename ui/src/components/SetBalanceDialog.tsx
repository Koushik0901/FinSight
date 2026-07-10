import { useState, useEffect } from "react";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useSetAccountBalance } from "../api/hooks/accounts";
import { userErrorMessage } from "../utils/runtime";
import type { AccountSummary } from "../api/client";

interface Props {
  open: boolean;
  onClose: () => void;
  account: AccountSummary | undefined;
}

export default function SetBalanceDialog({ open, onClose, account }: Props) {
  const [dollars, setDollars] = useState("");
  const setBalance = useSetAccountBalance();

  useEffect(() => {
    if (open) setDollars("");
  }, [open]);

  async function submit() {
    if (!account) return;
    const parsed = Number(dollars);
    if (!Number.isFinite(parsed)) {
      toast.error("Enter a valid balance amount.");
      return;
    }
    try {
      await setBalance.mutateAsync({ id: account.id, balanceCents: Math.round(parsed * 100) });
      toast.success(`Balance set for ${account.name}`);
      onClose();
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not set the balance. Try again."));
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title="Set current balance">
      <form
        className="drawer-form"
        onSubmit={(e) => {
          e.preventDefault();
          submit();
        }}
      >
        <p className="muted" style={{ marginTop: 0, marginBottom: 16, fontSize: 13.5 }}>
          {account
            ? `Imported history for ${account.name} doesn't include a balance. Enter what the account holds right now — FinSight anchors your imported transactions to it, so the balance stays correct and keeps tracking as you add activity.`
            : "Enter what the account holds right now."}
        </p>
        <label>
          Current balance ($)
          <input
            type="number"
            step="0.01"
            autoFocus
            value={dollars}
            onChange={(e) => setDollars(e.target.value)}
            placeholder="0.00"
          />
        </label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" className="primary" disabled={setBalance.isPending || dollars === ""}>
            {setBalance.isPending ? "Saving…" : "Save balance"}
          </button>
        </div>
      </form>
    </Drawer>
  );
}
