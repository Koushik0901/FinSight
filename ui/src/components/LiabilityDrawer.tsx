import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import {
  useCreateLiability, useUpdateLiability, useDeleteLiability,
} from "../api/hooks/assets";
import type { Liability } from "../api/client";

const LIABILITY_TYPES = ["mortgage", "loan", "credit-card", "other"] as const;

const optionalNonNegative = z.preprocess(
  (v) => (v === "" || v === undefined || v === null ? undefined : v),
  z.coerce.number().nonnegative().optional()
);

const schema = z.object({
  name: z.string().min(1, "Required"),
  liabilityType: z.enum(LIABILITY_TYPES),
  balance_dollars: z.coerce.number().nonnegative("Must be ≥ 0"),
  limit_dollars: optionalNonNegative,
  original_balance_dollars: optionalNonNegative,
  apr_pct: optionalNonNegative,
  min_payment_dollars: optionalNonNegative,
  payoff_date: z.string().optional(),
  started_at: z.string().optional(),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  liability?: Liability;
}

export default function LiabilityDrawer({ open, onClose, liability }: Props) {
  const isEdit = !!liability;
  const create = useCreateLiability();
  const update = useUpdateLiability();
  const del = useDeleteLiability();
  const [deleteConfirm, setDeleteConfirm] = useState(false);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", liabilityType: "loan", balance_dollars: 0 },
  });

  useEffect(() => {
    if (liability) {
      reset({
        name: liability.name,
        liabilityType: liability.liabilityType as typeof LIABILITY_TYPES[number],
        balance_dollars: liability.balanceCents / 100,
        limit_dollars: liability.limitCents != null ? liability.limitCents / 100 : undefined,
        original_balance_dollars: liability.originalBalanceCents != null ? liability.originalBalanceCents / 100 : undefined,
        apr_pct: liability.aprPct ?? undefined,
        min_payment_dollars: liability.minPaymentCents != null ? liability.minPaymentCents / 100 : undefined,
        payoff_date: liability.payoffDate ?? undefined,
        started_at: liability.startedAt ?? undefined,
      });
    } else {
      reset({ name: "", liabilityType: "loan", balance_dollars: 0 });
    }
    setDeleteConfirm(false);
  }, [liability?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  async function onSubmit(values: FormValues) {
    try {
      const balanceCents = Math.round(values.balance_dollars * 100);
      const limitCents = values.limit_dollars != null && !Number.isNaN(values.limit_dollars)
        ? Math.round(values.limit_dollars * 100) : null;
      const originalBalanceCents = values.original_balance_dollars != null && !Number.isNaN(values.original_balance_dollars)
        ? Math.round(values.original_balance_dollars * 100) : null;
      const aprPct = values.apr_pct != null && !Number.isNaN(values.apr_pct) ? values.apr_pct : null;
      const minPaymentCents = values.min_payment_dollars != null && !Number.isNaN(values.min_payment_dollars)
        ? Math.round(values.min_payment_dollars * 100) : null;
      const payoffDate = values.payoff_date || null;
      const startedAt = values.started_at || null;
      if (isEdit && liability) {
        await update.mutateAsync({
          id: liability.id,
          patch: {
            name: values.name, liabilityType: values.liabilityType, balanceCents,
            limitCents, originalBalanceCents, aprPct, minPaymentCents, payoffDate, startedAt, currency: null,
          },
        });
      } else {
        await create.mutateAsync({
          name: values.name, liabilityType: values.liabilityType, balanceCents,
          limitCents, originalBalanceCents, aprPct, minPaymentCents, payoffDate, startedAt, currency: "USD",
        });
      }
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not save liability");
    }
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!liability) return;
    try { await del.mutateAsync(liability.id); onClose(); }
    catch { setDeleteConfirm(false); }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit liability" : "Add liability"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Name
          <input {...register("name")} placeholder="e.g. Mortgage" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <label> Type
          <select {...register("liabilityType")}>
            {LIABILITY_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </label>
        <label> Balance ($)
          <input type="number" step="0.01" {...register("balance_dollars")} aria-invalid={!!errors.balance_dollars} />
          {errors.balance_dollars && <span className="err">{errors.balance_dollars.message}</span>}
        </label>
        <label> Credit limit / original ($) <input type="number" step="0.01" {...register("limit_dollars")} /></label>
        <label>
          Original balance ($)
          <input type="number" step="0.01" {...register("original_balance_dollars")} />
          {!watch("original_balance_dollars") && (
            <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
              Add an original balance to track payoff progress.
            </div>
          )}
        </label>
        <label> APR (%) <input type="number" step="0.01" {...register("apr_pct")} /></label>
        <label> Minimum payment ($) <input type="number" step="0.01" {...register("min_payment_dollars")} /></label>
        <label> Payoff date <input type="date" {...register("payoff_date")} /></label>
        <label>
          Started (month / year)
          <input type="month" {...register("started_at")} />
          {!watch("started_at") && (
            <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
              Add a start date to see how long you've been paying this down.
            </div>
          )}
        </label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : (isEdit ? "Save changes" : "Add liability")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button type="button" className="danger" onClick={handleDelete} disabled={del.isPending}>
            {deleteConfirm ? "Confirm delete?" : "Delete liability"}
          </button>
          {deleteConfirm && (
            <button type="button" onClick={() => setDeleteConfirm(false)} style={{ marginLeft: 8 }}>Cancel</button>
          )}
        </div>
      )}
    </Drawer>
  );
}
