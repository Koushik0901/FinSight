import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useCreateAccount, useUpdateAccount, useArchiveAccount } from "../api/hooks/accounts";
import {
  useAccountOwners,
  useCreateHouseholdMember,
  useHouseholdMembers,
  useSetAccountOwners,
} from "../api/hooks/household";
import type { Account } from "../api/bindings";

/** The fields the drawer actually edits — both the full `Account` and the
 *  list-page `AccountSummary` satisfy this, so either can open the editor. */
export type EditableAccount = Pick<
  Account,
  | "id" | "bank" | "name" | "type" | "currency" | "color" | "owner" | "apy_pct" | "nickname"
  | "apr_pct" | "min_payment_cents" | "payoff_date" | "limit_cents" | "original_balance_cents" | "started_at"
> & { last4?: string | null };
import { userErrorMessage } from "../utils/runtime";
import { accountTypeColor } from "../utils/accountColor";

const optionalNumber = z.preprocess(
  (v) => (v === "" || v === undefined || v === null ? undefined : v),
  z.coerce.number().nonnegative().optional()
);

const schema = z.object({
  bank: z.string().min(1, "Required"),
  name: z.string().min(1, "Required"),
  nickname: z.string().optional(),
  type: z.enum(["Checking", "Savings", "Credit", "Investment", "Cash", "Loan", "Other"]),
  last4: z.string().max(4).optional(),
  currency: z.enum(["USD", "EUR", "GBP", "CAD", "AUD"]),
  opening_dollars: z.coerce.number(),
  apy_pct: optionalNumber,
  // Debt fields — only meaningful for Credit/Loan accounts (shown conditionally).
  apr_pct: optionalNumber,
  min_payment_dollars: optionalNumber,
  limit_dollars: optionalNumber,
  original_balance_dollars: optionalNumber,
  payoff_date: z.string().optional(),
  started_at: z.string().optional(),
});

/// Avatar colors cycled for newly created household members.
const MEMBER_COLORS = ["#38BDF8", "#F472B6", "#4ADE80", "#FBBF24", "#C084FC", "#F87171"];

type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  account?: EditableAccount;
  /** Called after a successful create with the new account's id (so callers can
   *  auto-select it, e.g. the CSV import dialog). Not fired on edit. */
  onCreated?: (accountId: string) => void;
  /** Stack above an already-open dialog (inline creation from the import dialog). */
  elevated?: boolean;
}

export default function AccountDrawer({ open, onClose, account, onCreated, elevated }: Props) {
  const isEdit = !!account;
  const createAccount = useCreateAccount();
  const updateAccount = useUpdateAccount();
  const archiveAccount = useArchiveAccount();
  const [archiveConfirm, setArchiveConfirm] = useState(false);

  // Household ownership: pick zero or more members. 2+ selected = joint
  // account; none = shared household account.
  const { data: members = [] } = useHouseholdMembers();
  const { data: allOwners = [] } = useAccountOwners();
  const createMember = useCreateHouseholdMember();
  const setAccountOwners = useSetAccountOwners();
  const [selectedOwnerIds, setSelectedOwnerIds] = useState<string[]>([]);
  const [newPersonName, setNewPersonName] = useState("");

  const { register, handleSubmit, watch, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: {
      type: "Checking",
      currency: "USD",
      opening_dollars: 0,
      apy_pct: undefined,
      nickname: undefined,
      apr_pct: undefined,
      min_payment_dollars: undefined,
      limit_dollars: undefined,
      original_balance_dollars: undefined,
      payoff_date: undefined,
      started_at: undefined,
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
        opening_dollars: 0,
        apy_pct: account.apy_pct ?? undefined,
        nickname: account.nickname ?? undefined,
        apr_pct: account.apr_pct ?? undefined,
        min_payment_dollars: account.min_payment_cents != null ? account.min_payment_cents / 100 : undefined,
        limit_dollars: account.limit_cents != null ? account.limit_cents / 100 : undefined,
        original_balance_dollars: account.original_balance_cents != null ? account.original_balance_cents / 100 : undefined,
        payoff_date: account.payoff_date ?? undefined,
        started_at: account.started_at ?? undefined,
      });
      setSelectedOwnerIds(allOwners.filter((o) => o.accountId === account.id).map((o) => o.memberId));
    } else {
      reset({
        type: "Checking",
        currency: "USD",
        opening_dollars: 0,
        nickname: undefined,
        apr_pct: undefined,
        min_payment_dollars: undefined,
        limit_dollars: undefined,
        original_balance_dollars: undefined,
        payoff_date: undefined,
        started_at: undefined,
      });
      setSelectedOwnerIds([]);
    }
    setNewPersonName("");
    setArchiveConfirm(false);
  }, [account?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  const toggleOwner = (memberId: string) => {
    setSelectedOwnerIds((prev) =>
      prev.includes(memberId) ? prev.filter((id) => id !== memberId) : [...prev, memberId]
    );
  };

  const handleAddPerson = async () => {
    const name = newPersonName.trim();
    if (!name) return;
    try {
      const member = await createMember.mutateAsync({
        name,
        color: MEMBER_COLORS[members.length % MEMBER_COLORS.length],
      });
      setSelectedOwnerIds((prev) => [...prev, member.id]);
      setNewPersonName("");
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not add this person."));
    }
  };

  const ownerDisplay = (ids: string[]) => {
    const names = members.filter((m) => ids.includes(m.id)).map((m) => m.name);
    return names.length === 0 ? "Household" : names.join(" & ");
  };

  const isDebtType = (type: FormValues["type"]) => type === "Credit" || type === "Loan";

  /** apr_pct/min_payment_cents/payoff_date/limit_cents/original_balance_cents/
   *  started_at — only meaningful for Credit/Loan accounts. Cleared to null
   *  if the type isn't debt, even if stale values linger in the form. */
  function debtFieldsFromValues(values: FormValues, type: FormValues["type"]) {
    if (!isDebtType(type)) {
      return {
        apr_pct: null,
        min_payment_cents: null,
        payoff_date: null,
        limit_cents: null,
        original_balance_cents: null,
        started_at: null,
      };
    }
    return {
      apr_pct: values.apr_pct != null && !Number.isNaN(values.apr_pct) ? values.apr_pct : null,
      min_payment_cents: values.min_payment_dollars != null && !Number.isNaN(values.min_payment_dollars) ? Math.round(values.min_payment_dollars * 100) : null,
      payoff_date: values.payoff_date ? values.payoff_date : null,
      limit_cents: values.limit_dollars != null && !Number.isNaN(values.limit_dollars) ? Math.round(values.limit_dollars * 100) : null,
      original_balance_cents: values.original_balance_dollars != null && !Number.isNaN(values.original_balance_dollars) ? Math.round(values.original_balance_dollars * 100) : null,
      started_at: values.started_at ? values.started_at : null,
    };
  }

  async function onSubmit(values: FormValues) {
    try {
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
            liquidity_type: null,
            emergency_fund_eligible: null,
            goal_earmark: null,
            apy_pct: values.apy_pct != null && !Number.isNaN(values.apy_pct) ? values.apy_pct : null,
            nickname: values.nickname ? values.nickname : null,
            official_name: null,
            subtype: null,
            account_group: null,
            import_pending: null,
            ...debtFieldsFromValues(values, account.type),
          },
        });
        await setAccountOwners.mutateAsync({ accountId: account.id, memberIds: selectedOwnerIds });
      } else {
        const created = await createAccount.mutateAsync({
          bank: values.bank,
          name: values.name,
          type: values.type,
          last4: values.last4 || null,
          currency: values.currency,
          // Account color = its type's canonical color, so every surface that
          // shows the account inherits the type scheme automatically.
          color: accountTypeColor(values.type),
          opening_balance_cents: Math.round(values.opening_dollars * 100),
          owner: ownerDisplay(selectedOwnerIds),
          source: "manual",
          // Debt (Credit/Loan) is never liquid or emergency-fund eligible,
          // regardless of the account-level liquidity_type tag.
          liquidity_type: isDebtType(values.type) ? "restricted" : "liquid",
          emergency_fund_eligible: !isDebtType(values.type),
          goal_earmark: null,
          apy_pct: values.apy_pct != null && !Number.isNaN(values.apy_pct) ? values.apy_pct : null,
          simplefin_account_id: null,
          nickname: values.nickname ? values.nickname : null,
          connection_id: null,
          institution_id: null,
          external_account_id: null,
          official_name: null,
          mask: null,
          subtype: null,
          account_group: isDebtType(values.type) ? "debt" : "cash",
          available_balance_cents: null,
          ...debtFieldsFromValues(values, values.type),
          balance_date: null,
          extra_json: null,
          raw_json: null,
          import_pending: false,
        });
        if (selectedOwnerIds.length > 0) {
          await setAccountOwners.mutateAsync({ accountId: created.id, memberIds: selectedOwnerIds });
        }
        reset();
        onCreated?.(created.id);
        onClose();
        return;
      }
      reset();
      onClose();
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not save this account. Try again."));
    }
  }

  async function handleArchive() {
    if (!archiveConfirm) { setArchiveConfirm(true); return; }
    if (!account) return;
    try {
      await archiveAccount.mutateAsync(account.id);
      onClose();
    } catch (err) {
      toast.error(userErrorMessage(err, "Could not archive this account. Try again."));
      setArchiveConfirm(false);
    }
  }

  return (
    <Drawer open={open} onClose={onClose} elevated={elevated} title={isEdit ? "Edit Account" : "Add account"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Bank
          <input {...register("bank")} aria-invalid={!!errors.bank} />
          {errors.bank && <span className="err">{errors.bank.message}</span>}
        </label>
        <label> Name
          <input {...register("name")} placeholder="e.g. Joint Checking" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <label> Nickname <span className="muted">(optional)</span>
          <input {...register("nickname")} placeholder="e.g. Main Checking" />
        </label>
        {!isEdit && (
          <fieldset>
            <legend>Type</legend>
            {(["Checking","Savings","Credit","Investment","Cash","Loan","Other"] as const).map(t => (
              <label key={t}><input type="radio" value={t} {...register("type")} /> <span className="cswatch" style={{ background: accountTypeColor(t), width: 8, height: 8 }} /> {t}</label>
            ))}
          </fieldset>
        )}
        <label> Last 4 <input {...register("last4")} maxLength={4} /></label>
        <label> Currency
          <select {...register("currency")}>
            {(["USD","EUR","GBP","CAD","AUD"] as const).map(c => <option key={c}>{c}</option>)}
          </select>
        </label>
        {(watch("type") === "Savings" || (isEdit && account?.type === "Savings")) && (
          <label> APY (%)
            <input type="number" step="0.01" {...register("apy_pct")} />
            {!watch("apy_pct") && (
              <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
                Add an APY so savings projections use your real rate.
              </div>
            )}
          </label>
        )}
        {!isEdit && (
          <label> Opening balance ($)
            <input type="number" step="0.01" {...register("opening_dollars")} />
          </label>
        )}
        {isDebtType(watch("type")) && (
          <fieldset>
            <legend>Debt details <span className="muted" style={{ fontWeight: 400, fontSize: 12 }}>(optional)</span></legend>
            <label> APR (%)
              <input type="number" step="0.01" {...register("apr_pct")} />
            </label>
            <label> Minimum payment ($/mo)
              <input type="number" step="0.01" {...register("min_payment_dollars")} />
            </label>
            {watch("type") === "Credit" && (
              <label> Credit limit ($)
                <input type="number" step="0.01" {...register("limit_dollars")} />
              </label>
            )}
            <label> Original balance ($)
              <input type="number" step="0.01" {...register("original_balance_dollars")} />
            </label>
            <label> Started
              <input type="month" {...register("started_at")} />
            </label>
            <label> Payoff target date
              <input type="date" {...register("payoff_date")} />
            </label>
            <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
              Powers the debt payoff projector on Goals and Copilot debt questions.
            </div>
          </fieldset>
        )}
        <fieldset>
          <legend>
            Owners
            <span className="muted" style={{ fontWeight: 400, marginLeft: 8, fontSize: 12 }}>
              {selectedOwnerIds.length >= 2 ? "Joint account" : selectedOwnerIds.length === 1 ? "Sole account" : "Shared household account"}
            </span>
          </legend>
          {members.map((member) => (
            <label key={member.id}>
              <input
                type="checkbox"
                checked={selectedOwnerIds.includes(member.id)}
                onChange={() => toggleOwner(member.id)}
                aria-label={`Owner ${member.name}`}
              />{" "}
              <span className="cswatch" style={{ background: member.color || "var(--ink-faint)", width: 8, height: 8 }} /> {member.name}
            </label>
          ))}
          <div className="row row-sm" style={{ marginTop: 8 }}>
            <input
              placeholder="Add a person (e.g. Swathi)"
              value={newPersonName}
              onChange={(e) => setNewPersonName(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  void handleAddPerson();
                }
              }}
              aria-label="New household member name"
            />
            <button
              type="button"
              className="btn sm"
              disabled={!newPersonName.trim() || createMember.isPending}
              onClick={() => void handleAddPerson()}
            >
              Add person
            </button>
          </div>
          <div className="hint" style={{ marginTop: 6, fontSize: 12, color: "var(--ink-faint)" }}>
            Pick everyone who owns this account — two or more makes it a joint account.
          </div>
        </fieldset>
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
