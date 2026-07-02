import { useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useAccounts } from "../api/hooks/accounts";
import { useCategories } from "../api/hooks/transactions";
import {
  useDeletePlannedTransaction,
  useUpdatePlannedTransaction,
} from "../api/hooks/plannedTransactions";
import type { PlannedTransaction } from "../api/client";
import { money } from "../utils/format";
import { getAccountDisplayName } from "../utils/accounts";

interface Props {
  open: boolean;
  onClose: () => void;
  planned?: PlannedTransaction | null;
}

const STATUS_OPTIONS = ["planned", "completed", "skipped"] as const;

export default function PlannedTransactionDrawer({ open, onClose, planned }: Props) {
  const updatePlanned = useUpdatePlannedTransaction();
  const deletePlanned = useDeletePlannedTransaction();
  const { data: accounts = [] } = useAccounts();
  const { data: categories = [] } = useCategories();
  const [description, setDescription] = useState("");
  const [amount, setAmount] = useState("");
  const [dueDate, setDueDate] = useState("");
  const [status, setStatus] = useState<(typeof STATUS_OPTIONS)[number]>("planned");
  const [accountId, setAccountId] = useState("");
  const [categoryId, setCategoryId] = useState("");

  useEffect(() => {
    if (!planned) {
      setDescription("");
      setAmount("");
      setDueDate("");
      setStatus("planned");
      setAccountId("");
      setCategoryId("");
      return;
    }
    setDescription(planned.description);
    setAmount(String((planned.amountCents / 100).toFixed(2)));
    setDueDate(planned.dueDate);
    setStatus((STATUS_OPTIONS as readonly string[]).includes(planned.status) ? (planned.status as (typeof STATUS_OPTIONS)[number]) : "planned");
    setAccountId(planned.accountId ?? "");
    setCategoryId(planned.categoryId ?? "");
  }, [planned?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  const linkedAccount = useMemo(
    () => accounts.find((account) => account.id === planned?.accountId) ?? null,
    [accounts, planned?.accountId]
  );
  const linkedCategory = useMemo(
    () => categories.find((category) => category.id === planned?.categoryId) ?? null,
    [categories, planned?.categoryId]
  );

  async function save() {
    if (!planned) return;
    const amountCents = Math.round(Number(amount) * 100);
    if (!description.trim() || !dueDate || Number.isNaN(amountCents)) {
      toast.error("Description, amount, and due date are required");
      return;
    }

    try {
      await updatePlanned.mutateAsync({
        id: planned.id,
        patch: {
          description: description.trim(),
          amountCents,
          dueDate,
          status,
          accountId: accountId || null,
          categoryId: categoryId || null,
          source: planned.source,
        },
      });
      toast.success("Planned transaction updated");
      onClose();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Could not save planned transaction");
    }
  }

  async function remove() {
    if (!planned) return;
    try {
      await deletePlanned.mutateAsync(planned.id);
      toast.success("Planned transaction removed");
      onClose();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : "Could not delete planned transaction");
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title={planned ? `Planned transaction · ${planned.description}` : "Planned transaction"} width={560}>
      {!planned ? (
        <div className="stub">Select a planned transaction to edit it.</div>
      ) : (
        <div className="drawer-form">
          <div className="card tight" style={{ padding: 16, background: "var(--surface-2)" }}>
            <div className="row row-sm wrap" style={{ marginBottom: 10 }}>
              <span className="chip">{planned.status}</span>
              <span className="chip">{new Date(planned.dueDate).toLocaleDateString("en-US", { month: "short", day: "numeric", year: "numeric" })}</span>
              <span className="chip money">{money(planned.amountCents, { currency: "USD", decimals: 2 })}</span>
            </div>
            <div className="muted">Source: {planned.source}</div>
            {linkedAccount && <div className="muted" style={{ marginTop: 4 }}>Linked account: {getAccountDisplayName(linkedAccount)}</div>}
            {linkedCategory && <div className="muted" style={{ marginTop: 4 }}>Linked category: {linkedCategory.label}</div>}
          </div>

          <label>
            Description
            <input value={description} onChange={(e) => setDescription(e.target.value)} />
          </label>

          <label>
            Amount ($)
            <input type="number" step="0.01" value={amount} onChange={(e) => setAmount(e.target.value)} />
          </label>

          <label>
            Due date
            <input type="date" value={dueDate} onChange={(e) => setDueDate(e.target.value)} />
          </label>

          <label>
            Status
            <select value={status} onChange={(e) => setStatus(e.target.value as (typeof STATUS_OPTIONS)[number])}>
              {STATUS_OPTIONS.map((option) => (
                <option key={option} value={option}>{option}</option>
              ))}
            </select>
          </label>

          <label>
            Account
            <select value={accountId} onChange={(e) => setAccountId(e.target.value)}>
              <option value="">None</option>
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>{getAccountDisplayName(account)}</option>
              ))}
            </select>
          </label>

          <label>
            Category
            <select value={categoryId} onChange={(e) => setCategoryId(e.target.value)}>
              <option value="">None</option>
              {categories.map((category) => (
                <option key={category.id} value={category.id}>{category.label}</option>
              ))}
            </select>
          </label>

          <div className="form-actions">
            <button type="button" onClick={onClose}>Cancel</button>
            <button type="button" onClick={() => void remove()} disabled={deletePlanned.isPending}>Delete</button>
            <button type="button" className="primary" onClick={() => void save()} disabled={updatePlanned.isPending}>
              Save transaction
            </button>
          </div>
        </div>
      )}
    </Drawer>
  );
}
