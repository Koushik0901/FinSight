import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import Drawer from "./Drawer";
import { useCreateAccount } from "../api/hooks/accounts";

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
  defaultOwner?: string;
  onCreated?: () => void;
}

export default function AccountDrawer({ open, onClose, defaultOwner = "joint", onCreated }: Props) {
  const createAccount = useCreateAccount();
  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      type: "Checking",
      currency: "USD",
      owner: defaultOwner,
      opening_dollars: 0,
    },
  });

  async function onSubmit(values: FormValues) {
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
    reset();
    onCreated?.();
    onClose();
  }

  return (
    <Drawer open={open} onClose={onClose} title="Add account">
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Bank
          <input {...register("bank")} aria-invalid={!!errors.bank} />
          {errors.bank && <span className="err">{errors.bank.message}</span>}
        </label>
        <label> Name
          <input {...register("name")} placeholder="e.g. Joint Checking" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <fieldset>
          <legend>Type</legend>
          {(["Checking","Savings","Credit","Investment","Cash","Other"] as const).map(t => (
            <label key={t}><input type="radio" value={t} {...register("type")} /> {t}</label>
          ))}
        </fieldset>
        <label> Last 4 <input {...register("last4")} maxLength={4} /></label>
        <label> Currency
          <select {...register("currency")}>
            {(["USD","EUR","GBP","CAD","AUD"] as const).map(c => <option key={c}>{c}</option>)}
          </select>
        </label>
        <label> Opening balance ($)
          <input type="number" step="0.01" {...register("opening_dollars")} />
        </label>
        <label> Owner
          <input {...register("owner")} aria-invalid={!!errors.owner} />
        </label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Creating…" : "Create account"}
          </button>
        </div>
      </form>
    </Drawer>
  );
}
