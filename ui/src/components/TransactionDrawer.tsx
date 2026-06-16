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
  useTransactionSplits, useSetTransactionSplits,
} from "../api/hooks/transactions";
import { useAccounts } from "../api/hooks/accounts";
import type { Transaction } from "../api/bindings";

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
  const { data: accounts = [] } = useAccounts();
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
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
        });
        onCreated?.();
      }
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Something went wrong");
    }
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!transaction) return;
    try {
      await del.mutateAsync(transaction.id);
      onClose();
    } catch {
      setDeleteConfirm(false);
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit Transaction" : "Add transaction"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
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
          {transaction?.is_split ? (
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
              toast.error(err instanceof Error ? err.message : "Could not update flag");
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
                toast.error(err instanceof Error ? err.message : "Could not clear splits");
              }
            }
          }}
          >
            Split
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
