import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import Drawer from "./Drawer";
import { useCreateAccount, useUpdateAccount, useArchiveAccount } from "../api/hooks/accounts";
import type { Account } from "../api/bindings";

const schema = z.object({
  bank: z.string().min(1, "Required"),
  name: z.string().min(1, "Required"),
  type: z.enum(["Checking", "Savings", "Credit", "Investment", "Cash", "Other"]),
  last4: z.string().max(4).optional(),
  currency: z.enum(["USD", "EUR", "GBP", "CAD", "AUD"]),
  opening_dollars: z.coerce.number(),
  owner: z.string().min(1, "Required"),
});

type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  account?: Account;
  defaultOwner?: string;
  onCreated?: () => void;
}

export default function AccountDrawer({ open, onClose, account, defaultOwner = "joint", onCreated }: Props) {
  const isEdit = !!account;
  const createAccount = useCreateAccount();
  const updateAccount = useUpdateAccount();
  const archiveAccount = useArchiveAccount();
  const [archiveConfirm, setArchiveConfirm] = useState(false);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      type: "Checking",
      currency: "USD",
      owner: defaultOwner,
      opening_dollars: 0,
    },
  });

  useEffect(() => {
    if (account) {
      reset({
        bank: account.bank,
        name: account.name,
        type: account.type,
        last4: account.last4 ?? undefined,
        currency: account.currency as "USD" | "EUR" | "GBP" | "CAD" | "AUD",
        owner: account.owner,
        opening_dollars: 0,
      });
    } else {
      reset({ type: "Checking", currency: "USD", owner: defaultOwner, opening_dollars: 0 });
    }
    setArchiveConfirm(false);
  }, [account?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  async function onSubmit(values: FormValues) {
    if (isEdit && account) {
      await updateAccount.mutateAsync({
        id: account.id,
        patch: {
          name: values.name,
          bank: values.bank,
          account_type: null,
          color: account.color,
          currency: values.currency,
          last4: values.last4 ? values.last4 : null,
        },
      });
    } else {
      await createAccount.mutateAsync({
        bank: values.bank,
        name: values.name,
        type: values.type,
        last4: values.last4 || null,
        currency: values.currency,
        color: "#3B82F6",
        opening_balance_cents: Math.round(values.opening_dollars * 100),
        owner: values.owner,
        source: "manual",
      });
    }
    reset();
    onCreated?.();
    onClose();
  }

  async function handleArchive() {
    if (!archiveConfirm) { setArchiveConfirm(true); return; }
    if (!account) return;
    try {
      await archiveAccount.mutateAsync(account.id);
      onClose();
    } catch {
      setArchiveConfirm(false);
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit Account" : "Add account"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Bank
          <input {...register("bank")} aria-invalid={!!errors.bank} />
          {errors.bank && <span className="err">{errors.bank.message}</span>}
        </label>
        <label> Name
          <input {...register("name")} placeholder="e.g. Joint Checking" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        {!isEdit && (
          <fieldset>
            <legend>Type</legend>
            {(["Checking","Savings","Credit","Investment","Cash","Other"] as const).map(t => (
              <label key={t}><input type="radio" value={t} {...register("type")} /> {t}</label>
            ))}
          </fieldset>
        )}
        <label> Last 4 <input {...register("last4")} maxLength={4} /></label>
        <label> Currency
          <select {...register("currency")}>
            {(["USD","EUR","GBP","CAD","AUD"] as const).map(c => <option key={c}>{c}</option>)}
          </select>
        </label>
        {!isEdit && (
          <label> Opening balance ($)
            <input type="number" step="0.01" {...register("opening_dollars")} />
          </label>
        )}
        {!isEdit && (
          <label> Owner
            <input {...register("owner")} aria-invalid={!!errors.owner} />
          </label>
        )}
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? (isEdit ? "Saving…" : "Creating…") : (isEdit ? "Save changes" : "Create account")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button
            type="button"
            className="danger"
            onClick={handleArchive}
            disabled={archiveAccount.isPending}
          >
            {archiveConfirm ? "Confirm archive?" : "Archive account"}
          </button>
          {archiveConfirm && (
            <button type="button" onClick={() => setArchiveConfirm(false)} style={{ marginLeft: 8 }}>
              Cancel
            </button>
          )}
        </div>
      )}
    </Drawer>
  );
}
