import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import CategoryPicker from "./CategoryPicker";
import SplitModal from "./SplitModal";
import {
  useCreateTransaction, useUpdateTransaction,
  useDeleteTransaction, useCreateRule, useSetTransactionFlags,
  useTransactionSplits, useSetTransactionSplits, useSetAnomalyDismissed,
  useSetTransactionOwner, useSetTransactionTransfer, useApplyTransferVerdictToSimilar,
} from "../api/hooks/transactions";
import { useAccounts } from "../api/hooks/accounts";
import { useAccountOwners, useHouseholdMembers } from "../api/hooks/household";
import type { Transaction } from "../api/bindings";
import { userErrorMessage } from "../utils/runtime";

const schema = z.object({
  merchant_raw: z.string().min(1, "Required"),
  amount_dollars: z.coerce.number(),
  notes: z.string().optional(),
  posted_at: z.string().min(1, "Required"),
  account_id: z.string().optional(),
});

type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  transaction?: Transaction;
  accountId?: string;
  onCreated?: () => void;
}

export default function TransactionDrawer({ open, onClose, transaction, accountId, onCreated }: Props) {
  const isEdit = !!transaction;
  const create = useCreateTransaction();
  const update = useUpdateTransaction();
  const del = useDeleteTransaction();
  const createRule = useCreateRule();
  const setFlags = useSetTransactionFlags();
  const setTransfer = useSetTransactionTransfer();
  const applySimilar = useApplyTransferVerdictToSimilar();
  const dismissAnomaly = useSetAnomalyDismissed();
  const { data: accounts = [] } = useAccounts();
  const setOwner = useSetTransactionOwner();
  const { data: allOwners = [] } = useAccountOwners();
  const { data: members = [] } = useHouseholdMembers();
  // Owners of THIS transaction's account. A 2+ owner (joint) account is where a
  // per-transaction attribution override is meaningful (a personal purchase on
  // the joint card).
  const accountOwners = transaction
    ? allOwners
        .filter((o) => o.accountId === transaction.account_id)
        .map((o) => members.find((m) => m.id === o.memberId))
        .filter((m): m is NonNullable<typeof m> => !!m)
    : [];
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [ownerId, setOwnerId] = useState<string | null>(null);
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [splitModalOpen, setSplitModalOpen] = useState(false);
  const { data: existingSplits = [] } = useTransactionSplits(transaction?.id);
  const clearSplits = useSetTransactionSplits();

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      merchant_raw: "",
      amount_dollars: 0,
      notes: "",
      posted_at: new Date().toISOString().slice(0, 10),
      account_id: accountId ?? "",
    },
  });

  useEffect(() => {
    if (transaction) {
      reset({
        merchant_raw: transaction.merchant_raw,
        amount_dollars: transaction.amount_cents / 100,
        notes: transaction.notes ?? "",
        posted_at: transaction.posted_at.slice(0, 10),
        account_id: transaction.account_id,
      });
      setSelectedCategory(transaction.category_id ?? null);
      setOwnerId(transaction.owner_member_id ?? null);
    } else {
      reset({
        merchant_raw: "",
        amount_dollars: 0,
        notes: "",
        posted_at: new Date().toISOString().slice(0, 10),
        account_id: accountId ?? "",
      });
      setSelectedCategory(null);
    }
    setDeleteConfirm(false);
  }, [transaction?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  async function onSubmit(values: FormValues) {
    try {
      if (isEdit && transaction) {
        const result = await update.mutateAsync({
          id: transaction.id,
          patch: {
            notes: values.notes || null,
            category_id: selectedCategory,
            merchant_raw: values.merchant_raw,
            amount_cents: Math.round(values.amount_dollars * 100),
            ai_confidence: null,
          },
        });
        if (result.proposed_rule) {
          const { pattern, category_id, category_label } = result.proposed_rule;
          toast.custom((t) => (
            <div role="alert">
              Always categorize <strong>«{pattern}»</strong> as{" "}
              <strong>{category_label}</strong>?{" "}
              <button type="button" onClick={() => { createRule.mutate({ pattern, categoryId: category_id }); toast.dismiss(t); }}>
                Create rule
              </button>{" "}
              <button type="button" onClick={() => toast.dismiss(t)}>Skip</button>
            </div>
          ));
        }
      } else {
        await create.mutateAsync({
          account_id: values.account_id ?? accountId ?? "",
          posted_at: new Date(values.posted_at + "T12:00:00Z").toISOString(),
          amount_cents: Math.round(values.amount_dollars * 100),
          merchant_raw: values.merchant_raw,
          category_id: selectedCategory,
          notes: values.notes || null,
          status: "manual",
          imported_id: null,
          source: "manual",
          raw_synced_data: null,
          pending: false,
          external_tx_id: null,
          external_account_id: null,
        });
        onCreated?.();
      }
      onClose();
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not save this transaction. Try again."));
    }
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!transaction) return;
    try {
      await del.mutateAsync(transaction.id);
      onClose();
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not delete this transaction. Try again."));
      setDeleteConfirm(false);
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit Transaction" : "Add transaction"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        {isEdit && transaction?.is_anomaly && (
          <div className="card tight" style={{ padding: 12, borderLeft: "3px solid var(--negative)", marginBottom: 4 }}>
            <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 4 }}>Flagged as unusual</div>
            {transaction.ai_explanation && (
              <div className="muted" style={{ fontSize: 12.5, marginBottom: 8 }}>{transaction.ai_explanation}</div>
            )}
            <button
              type="button"
              className="btn outline sm"
              disabled={dismissAnomaly.isPending}
              onClick={async () => {
                try {
                  await dismissAnomaly.mutateAsync({ txnId: transaction.id, dismissed: true });
                  toast.success("Marked as not unusual", { description: "It won't be flagged again." });
                  onClose();
                } catch (err) {
                  toast.error(userErrorMessage(err, "Could not update this charge."));
                }
              }}
            >
              This is fine — don't flag it
            </button>
          </div>
        )}
        {isEdit && transaction?.is_transfer && (
          <div className="card tight" style={{ padding: 12, borderLeft: "3px solid var(--accent)", marginBottom: 4 }}>
            <div style={{ fontSize: 13, fontWeight: 600, marginBottom: 4 }}>Transfer between your accounts</div>
            <div className="muted" style={{ fontSize: 12.5, marginBottom: 8 }}>
              {transaction.transfer_peer_account_name
                ? `Matched with the opposite leg in ${transaction.transfer_peer_account_name}. `
                : ""}
              It doesn't count as income or spending.
            </div>
          </div>
        )}
        <label> Merchant
          <input {...register("merchant_raw")} aria-invalid={!!errors.merchant_raw} />
          {errors.merchant_raw && <span className="err">{errors.merchant_raw.message}</span>}
        </label>
        <label> Amount ($)
          <input type="number" step="0.01" {...register("amount_dollars")} />
        </label>
        <label> Date
          <input type="date" {...register("posted_at")} />
        </label>
        <label> Notes
          <input {...register("notes")} />
        </label>
        {!isEdit && (
          <label> Account
            <select {...register("account_id")}>
              <option value="">— Pick an account —</option>
              {accounts.map((a) => <option key={a.id} value={a.id}>{a.bank} · {a.name}</option>)}
            </select>
          </label>
        )}
        <div>
          <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>Category</div>
          {transaction?.is_transfer ? (
            <div className="muted" style={{ fontSize: 12.5, padding: "8px 10px", background: "var(--surface-2)", borderRadius: 7, border: "1px solid var(--line)" }}>
              Transfers aren't categorized — this is money moved between your accounts, not spending.
            </div>
          ) : transaction?.is_split ? (
            <div style={{ display: "flex", alignItems: "center", gap: 8, padding: "8px 10px", background: "var(--surface-2)", borderRadius: 7, border: "1px solid var(--line)" }}>
              <span style={{ flex: 1, fontSize: 13, color: "var(--ink-mute)" }}>
                Split · {existingSplits.length} {existingSplits.length === 1 ? "category" : "categories"} · ${(Math.abs(transaction.amount_cents) / 100).toFixed(2)}
              </span>
              <button type="button" onClick={() => setSplitModalOpen(true)} style={{ fontSize: 12, color: "var(--accent)", background: "none", border: "none", cursor: "pointer", padding: 0 }}>
                Edit splits →
              </button>
            </div>
          ) : (
            <CategoryPicker value={selectedCategory} onChange={setSelectedCategory} />
          )}
        </div>
        {isEdit && transaction && accountOwners.length >= 2 && (
          <div>
            <div style={{ fontSize: 13, fontWeight: 500, marginBottom: 4 }}>Attributed to</div>
            <select
              value={ownerId ?? ""}
              onChange={(e) => {
                const memberId = e.target.value || null;
                setOwnerId(memberId);
                setOwner.mutate({ transactionId: transaction.id, memberId });
              }}
              aria-label="Attribute this transaction to"
              style={{ width: "100%" }}
            >
              <option value="">Shared — split by account ownership</option>
              {accountOwners.map((m) => (
                <option key={m.id} value={m.id}>
                  {m.name}
                </option>
              ))}
            </select>
            <div className="hint" style={{ fontSize: 12, color: "var(--ink-faint)", marginTop: 4 }}>
              On a joint account, attribute a personal purchase to one person so it counts as only
              their spending.
            </div>
          </div>
        )}
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : (isEdit ? "Save changes" : "Save transaction")}
          </button>
        </div>
      </form>
      {isEdit && transaction && (
        <div style={{ marginTop: 16, display: "flex", gap: 8 }}>
          <button
            type="button"
            className={`chip${transaction.is_reimbursable ? " accent" : ""}`}
            aria-pressed={transaction.is_reimbursable}
            onClick={async () => {
            try {
              await setFlags.mutateAsync({ id: transaction.id, isReimbursable: !transaction.is_reimbursable, isSplit: transaction.is_split });
            } catch (err) {
              toast.error(userErrorMessage(err, "Could not update this flag. Try again."));
            }
          }}
          >
            Reimbursable
          </button>
          <button
            type="button"
            className={`chip${transaction.is_split ? " accent" : ""}`}
            aria-pressed={transaction.is_split}
            onClick={async () => {
            if (!transaction.is_split) {
              // Turning ON: open SplitModal to define splits
              setSplitModalOpen(true);
            } else {
              // Turning OFF: clear splits
              try {
                await clearSplits.mutateAsync({ txnId: transaction.id, splits: [] });
              } catch (err) {
                toast.error(userErrorMessage(err, "Could not clear splits. Try again."));
              }
            }
          }}
          >
            Split
          </button>
          <button
            type="button"
            className={`chip${transaction.is_transfer ? " accent" : ""}`}
            aria-pressed={transaction.is_transfer}
            disabled={setTransfer.isPending}
            title="A transfer moves money between your own accounts and never counts as income or spending"
            onClick={async () => {
              const next = !transaction.is_transfer;
              try {
                const result = await setTransfer.mutateAsync({ id: transaction.id, isTransfer: next });
                const description = next
                  ? "It no longer counts as income or spending, and this won't be undone by future imports."
                  : "It now counts in your income and spending, and this won't be undone by future imports.";
                if (result?.similarPattern && result.similarCount > 0) {
                  // One decision can clear the whole counterparty from review.
                  const pattern = result.similarPattern;
                  const n = result.similarCount;
                  toast.success(next ? "Marked as a transfer" : "Marked as not a transfer", {
                    description,
                    action: {
                      label: `Also mark ${n} more with «${result.similarLabel}»`,
                      onClick: async () => {
                        try {
                          const applied = await applySimilar.mutateAsync({ pattern, isTransfer: next });
                          toast.success(`Marked ${applied} transaction${applied === 1 ? "" : "s"} the same way`);
                        } catch (err) {
                          toast.error(userErrorMessage(err, "Could not update the similar transactions."));
                        }
                      },
                    },
                  });
                } else {
                  toast.success(next ? "Marked as a transfer" : "Marked as not a transfer", { description });
                }
              } catch (err) {
                toast.error(userErrorMessage(err, "Could not update this transaction. Try again."));
              }
            }}
          >
            Transfer
          </button>
        </div>
      )}
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button type="button" className="danger" onClick={handleDelete} disabled={del.isPending}>
            {deleteConfirm ? "Confirm delete?" : "Delete transaction"}
          </button>
          {deleteConfirm && (
            <button type="button" onClick={() => setDeleteConfirm(false)} style={{ marginLeft: 8 }}>
              Cancel
            </button>
          )}
        </div>
      )}
      {transaction && (
        <SplitModal
          open={splitModalOpen}
          onClose={() => setSplitModalOpen(false)}
          transactionId={transaction.id}
          totalCents={Math.abs(transaction.amount_cents)}
          existingSplits={existingSplits}
        />
      )}
    </Drawer>
  );
}
