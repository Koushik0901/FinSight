import { useState, useEffect } from "react";
import { useForm } from "react-hook-form";
import { zodResolver } from "@hookform/resolvers/zod";
import { z } from "zod";
import { toast } from "sonner";
import Drawer from "./Drawer";
import {
  useCreateManualAsset, useUpdateManualAsset, useDeleteManualAsset,
} from "../api/hooks/assets";
import type { ManualAsset } from "../api/bindings";

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
  const [deleteConfirm, setDeleteConfirm] = useState(false);

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
    } else {
      reset({ name: "", assetType: "cash", value_dollars: 0, notes: "" });
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
      } else {
        await create.mutateAsync({
          name: values.name, assetType: values.assetType, valueCents,
          currency: "USD", notes: values.notes || null,
        });
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
