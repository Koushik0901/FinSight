import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import {
  useCreateManualAsset, useUpdateManualAsset, useDeleteManualAsset,
} from "../api/hooks/assets";
import {
  useHouseholdMembers, useAssetOwners, useSetAssetOwners,
} from "../api/hooks/household";
import type { ManualAsset } from "../api/client";

const ASSET_TYPES = ["cash", "property", "vehicle", "investment", "crypto", "other"] as const;

const schema = z.object({
  name: z.string().min(1, "Required"),
  assetType: z.enum(ASSET_TYPES),
  value_dollars: z.coerce.number().nonnegative("Must be ≥ 0"),
  notes: z.string().optional(),
});
type FormValues = z.infer<typeof schema>;

interface Props {
  open: boolean;
  onClose: () => void;
  asset?: ManualAsset;
}

export default function AssetDrawer({ open, onClose, asset }: Props) {
  const isEdit = !!asset;
  const create = useCreateManualAsset();
  const update = useUpdateManualAsset();
  const del = useDeleteManualAsset();
  const { data: members = [] } = useHouseholdMembers();
  const { data: allAssetOwners = [] } = useAssetOwners();
  const setAssetOwners = useSetAssetOwners();
  const [deleteConfirm, setDeleteConfirm] = useState(false);
  const [selectedOwnerIds, setSelectedOwnerIds] = useState<string[]>([]);
  const [ownerShares, setOwnerShares] = useState<Record<string, string>>({});

  // Persist asset ownership (same semantics as accounts): joint = explicit
  // shares, blank % = equal split, sub-100% total leaves the rest in the
  // household residual (owned in another person's separate app).
  const persistOwners = async (assetId: string) => {
    const owners = selectedOwnerIds.map((memberId) => {
      const raw = ownerShares[memberId]?.trim();
      const pct = raw ? Number(raw) : NaN;
      return { memberId, shareBps: Number.isFinite(pct) ? Math.round(pct * 100) : null };
    });
    await setAssetOwners.mutateAsync({ assetId, owners });
  };
  const toggleOwner = (id: string) =>
    setSelectedOwnerIds((prev) => (prev.includes(id) ? prev.filter((x) => x !== id) : [...prev, id]));

  const { register, handleSubmit, formState: { errors, isSubmitting }, reset } = useForm<FormValues>({
    resolver: zodResolver(schema),
    defaultValues: { name: "", assetType: "cash", value_dollars: 0, notes: "" },
  });

  useEffect(() => {
    if (asset) {
      reset({
        name: asset.name,
        assetType: asset.assetType as typeof ASSET_TYPES[number],
        value_dollars: asset.valueCents / 100,
        notes: asset.notes ?? "",
      });
      const owners = allAssetOwners.filter((o) => o.assetId === asset.id);
      setSelectedOwnerIds(owners.map((o) => o.memberId));
      setOwnerShares(
        Object.fromEntries(
          owners
            .filter((o) => o.shareBps != null)
            .map((o) => [o.memberId, String((o.shareBps as number) / 100)]),
        ),
      );
    } else {
      reset({ name: "", assetType: "cash", value_dollars: 0, notes: "" });
      setSelectedOwnerIds([]);
      setOwnerShares({});
    }
    setDeleteConfirm(false);
  }, [asset?.id, open]); // eslint-disable-line react-hooks/exhaustive-deps

  async function onSubmit(values: FormValues) {
    try {
      const valueCents = Math.round(values.value_dollars * 100);
      if (isEdit && asset) {
        await update.mutateAsync({
          id: asset.id,
          patch: {
            name: values.name, assetType: values.assetType, valueCents,
            currency: null, notes: values.notes || null,
          },
        });
        await persistOwners(asset.id);
      } else {
        const created = await create.mutateAsync({
          name: values.name, assetType: values.assetType, valueCents,
          currency: "USD", notes: values.notes || null,
        });
        if (selectedOwnerIds.length > 0) await persistOwners(created.id);
      }
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not save asset");
    }
  }

  async function handleDelete() {
    if (!deleteConfirm) { setDeleteConfirm(true); return; }
    if (!asset) return;
    try { await del.mutateAsync(asset.id); onClose(); }
    catch { setDeleteConfirm(false); }
  }

  return (
    <Drawer open={open} onClose={onClose} title={isEdit ? "Edit asset" : "Add manual asset"}>
      <form onSubmit={handleSubmit(onSubmit)} className="drawer-form">
        <label> Name
          <input {...register("name")} placeholder="e.g. Home" aria-invalid={!!errors.name} />
          {errors.name && <span className="err">{errors.name.message}</span>}
        </label>
        <label> Type
          <select {...register("assetType")}>
            {ASSET_TYPES.map((t) => <option key={t} value={t}>{t}</option>)}
          </select>
        </label>
        <label> Value ($)
          <input type="number" step="0.01" {...register("value_dollars")} aria-invalid={!!errors.value_dollars} />
          {errors.value_dollars && <span className="err">{errors.value_dollars.message}</span>}
        </label>
        <label> Notes <input {...register("notes")} /></label>
        {members.length > 0 && (
          <fieldset>
            <legend>
              Owners
              <span className="muted" style={{ fontWeight: 400, marginLeft: 8, fontSize: 12 }}>
                {selectedOwnerIds.length >= 2 ? "Jointly owned" : selectedOwnerIds.length === 1 ? "Sole" : "Household"}
              </span>
            </legend>
            {members.map((m) => (
              <label key={m.id} style={{ display: "block" }}>
                <input
                  type="checkbox"
                  checked={selectedOwnerIds.includes(m.id)}
                  onChange={() => toggleOwner(m.id)}
                  aria-label={`Owner ${m.name}`}
                />{" "}
                {m.name}
              </label>
            ))}
            {selectedOwnerIds.length >= 2 && (
              <div style={{ marginTop: 8 }}>
                <div className="muted" style={{ fontSize: 12, marginBottom: 4 }}>
                  Ownership split — leave a % blank for an equal share
                </div>
                {selectedOwnerIds.map((id) => {
                  const m = members.find((mm) => mm.id === id);
                  return (
                    <div key={id} className="row row-sm" style={{ alignItems: "center", gap: 6 }}>
                      <span style={{ flex: 1, fontSize: 13 }}>{m?.name ?? "Member"}</span>
                      <input
                        type="number"
                        min={0}
                        max={100}
                        step={1}
                        value={ownerShares[id] ?? ""}
                        onChange={(e) => setOwnerShares((prev) => ({ ...prev, [id]: e.target.value }))}
                        aria-label={`Ownership percent for ${m?.name ?? "member"}`}
                        style={{ width: 72 }}
                      />
                      <span style={{ fontSize: 12 }}>%</span>
                    </div>
                  );
                })}
              </div>
            )}
            <div className="hint" style={{ fontSize: 12, color: "var(--ink-faint)", marginTop: 4 }}>
              A jointly-owned asset folds each owner’s share into their net worth.
            </div>
          </fieldset>
        )}
        <div className="form-actions">
          <button type="button" onClick={onClose}>Cancel</button>
          <button type="submit" disabled={isSubmitting} className="primary">
            {isSubmitting ? "Saving…" : (isEdit ? "Save changes" : "Add asset")}
          </button>
        </div>
      </form>
      {isEdit && (
        <div style={{ marginTop: 24, paddingTop: 16, borderTop: "1px solid var(--hairline)" }}>
          <button type="button" className="danger" onClick={handleDelete} disabled={del.isPending}>
            {deleteConfirm ? "Confirm delete?" : "Delete asset"}
          </button>
          {deleteConfirm && (
            <button type="button" onClick={() => setDeleteConfirm(false)} style={{ marginLeft: 8 }}>Cancel</button>
          )}
        </div>
      )}
    </Drawer>
  );
}
