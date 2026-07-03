import { useState, useEffect } from "react";
import { useNavigate } from "react-router-dom";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useDeleteAllData } from "../api/hooks/settings";
import { useResetOnboarding } from "../api/hooks/onboarding";
import { userErrorMessage } from "../utils/runtime";

interface Props {
  open: boolean;
  onClose: () => void;
}

const CONFIRM_WORD = "DELETE";

export default function DeleteAllDataDialog({ open, onClose }: Props) {
  const navigate = useNavigate();
  const [typed, setTyped] = useState("");
  const deleteAll = useDeleteAllData();
  const resetOnboarding = useResetOnboarding();

  useEffect(() => {
    if (open) setTyped("");
  }, [open]);

  const canConfirm = typed.trim().toUpperCase() === CONFIRM_WORD;

  async function confirmDelete() {
    if (!canConfirm) return;
    try {
      await deleteAll.mutateAsync();
      // Send the user back through onboarding so the app truthfully reflects
      // its now-empty state rather than a dashboard full of stale zeros.
      try {
        await resetOnboarding.mutateAsync();
      } catch {
        // Non-fatal: the wipe already succeeded; onboarding flag is cosmetic.
      }
      toast.success("All local data deleted", {
        description: "Your financial data has been removed. Provider settings were kept.",
      });
      onClose();
      navigate("/onboarding");
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not delete data. Try again from the desktop app."));
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title="Delete all data">
      <div className="drawer-form">
        <div
          className="card"
          style={{
            padding: 14,
            marginBottom: 16,
            borderColor: "var(--negative)",
            background: "color-mix(in srgb, var(--negative) 8%, transparent)",
          }}
        >
          <div style={{ fontWeight: 600, color: "var(--negative)", marginBottom: 6 }}>
            This permanently deletes all local financial data.
          </div>
          <div className="muted" style={{ fontSize: 13, lineHeight: 1.55 }}>
            Every account, transaction, balance, budget, goal, category, scenario, recipe,
            insight, review item, and agent memory on this device will be removed. This cannot
            be undone. Your AI provider settings and API keys are kept.
          </div>
        </div>

        <label>
          Type <strong>{CONFIRM_WORD}</strong> to confirm
          <input
            type="text"
            autoFocus
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            placeholder={CONFIRM_WORD}
            aria-label={`Type ${CONFIRM_WORD} to confirm`}
            autoComplete="off"
            spellCheck={false}
          />
        </label>

        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button
            type="button"
            className="danger"
            disabled={!canConfirm || deleteAll.isPending}
            onClick={confirmDelete}
          >
            {deleteAll.isPending ? "Deleting…" : "Delete all data"}
          </button>
        </div>
      </div>
    </Drawer>
  );
}
