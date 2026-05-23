import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import Drawer from "./Drawer";
import { useCreateTransaction } from "../api/hooks/transactions";
import { useAccounts } from "../api/hooks/accounts";

const schema = z.object({
  account_id: z.string().min(1, "Required"),
  date: z.string().min(1),
  dollars: z.coerce.number(),
  direction: z.enum(["inflow", "outflow"]),
  merchant_raw: z.string().min(1, "Required"),
  category_id: z.string().optional(),
  notes: z.string().optional(),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  defaultAccountId?: string;
  onCreated?: () => void;
}

export default function TransactionDrawer({ open, onClose, defaultAccountId, onCreated }: Props) {
  const create = useCreateTransaction();
  const { data: accounts = [] } = useAccounts();

  const today = new Date().toISOString().slice(0, 10);

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      account_id: defaultAccountId ?? "",
      date: today,
      direction: "outflow",
    },
  });

  async function onSubmit(values: FormValues) {
    const cents_signed =
      values.direction === "outflow"
        ? -Math.round(Math.abs(values.dollars) * 100)
        :  Math.round(Math.abs(values.dollars) * 100);
    await create.mutateAsync({
      account_id: values.account_id,
      posted_at: new Date(values.date + "T12:00:00Z").toISOString(),
      amount_cents: cents_signed,
      merchant_raw: values.merchant_raw,
      category_id: values.category_id || null,
      notes: values.notes || null,
      status: "manual",
    });
    reset();
    onCreated?.();
    onClose();
  }

  return (
    <Drawer open={open} onClose={onClose} title="Add transaction">
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Account
          <select {...register("account_id")} aria-invalid={!!errors.account_id}>
            <option value="">— Pick an account —</option>
            {accounts.map(a => <option key={a.id} value={a.id}>{a.bank} · {a.name}</option>)}
          </select>
        </label>
        <label> Date
          <input type="date" {...register("date")} />
        </label>
        <fieldset>
          <legend>Direction</legend>
          <label><input type="radio" value="outflow" {...register("direction")} /> Outflow</label>
          <label><input type="radio" value="inflow"  {...register("direction")} /> Inflow</label>
        </fieldset>
        <label> Amount ($)
          <input type="number" step="0.01" {...register("dollars")} />
        </label>
        <label> Merchant
          <input {...register("merchant_raw")} aria-invalid={!!errors.merchant_raw} />
        </label>
        <label> Notes
          <textarea {...register("notes")} />
        </label>
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : "Save transaction"}
          </button>
        </div>
      </form>
    </Drawer>
  );
}
