import { useState, useEffect } from "react";
import { toast } from "sonner";
import Drawer from "./Drawer";
import { useSetTransactionSplits, useCategories } from "../api/hooks/transactions";
import type { TransactionSplitDto } from "../api/bindings";

interface SplitRow {
  id: string;
  categoryId: string | null;
  amountCents: number;
}

interface Props {
  open: boolean;
  onClose: () => void;
  transactionId: string;
  totalCents: number;
  existingSplits: TransactionSplitDto[];
}

function newRow(): SplitRow {
  return { id: crypto.randomUUID(), categoryId: null, amountCents: 0 };
}

export default function SplitModal({ open, onClose, transactionId, totalCents, existingSplits }: Props) {
  const setSplits = useSetTransactionSplits();
  const { data: categories = [] } = useCategories();
  const [rows, setRows] = useState<SplitRow[]>([newRow(), newRow()]);

  useEffect(() => {
    if (!open) return;
    if (existingSplits.length >= 2) {
      setRows(existingSplits.map(s => ({
        id: s.id,
        categoryId: s.categoryId ?? null,
        amountCents: s.amountCents,
      })));
    } else {
      setRows([newRow(), newRow()]);
    }
  }, [open, existingSplits]);

  const assigned = rows.reduce((sum, r) => sum + r.amountCents, 0);
  const remaining = totalCents - assigned;
  const pct = Math.min(100, totalCents > 0 ? (assigned / totalCents) * 100 : 0);
  const balanced = assigned === totalCents && totalCents > 0;

  function updateRow(id: string, patch: Partial<SplitRow>) {
    setRows(prev => prev.map(r => r.id === id ? { ...r, ...patch } : r));
  }

  function removeRow(id: string) {
    if (rows.length <= 2) return;
    setRows(prev => prev.filter(r => r.id !== id));
  }

  async function handleSave() {
    if (!balanced) {
      toast.error(`Splits must sum to $${(totalCents / 100).toFixed(2)}`);
      return;
    }
    try {
      await setSplits.mutateAsync({
        txnId: transactionId,
        splits: rows.map(r => ({ categoryId: r.categoryId, amountCents: r.amountCents })),
      });
      toast.success("Splits saved");
      onClose();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Could not save splits");
    }
  }

  return (
    <Drawer open={open} onClose={onClose} title="Split transaction">
      <div style={{ marginBottom: 8, fontSize: 13, color: "var(--ink-mute)" }}>
        Total: ${(totalCents / 100).toFixed(2)}
      </div>

      {/* Balance bar */}
      <div style={{ height: 6, background: "var(--line)", borderRadius: 3, marginBottom: 4, overflow: "hidden" }}>
        <div style={{
          height: "100%",
          width: `${pct}%`,
          background: balanced ? "var(--accent)" : "var(--negative)",
          borderRadius: 3,
          transition: "width 0.15s",
        }} />
      </div>
      <div style={{ display: "flex", justifyContent: "space-between", fontSize: 12, color: "var(--ink-mute)", marginBottom: 16 }}>
        <span>${(assigned / 100).toFixed(2)} assigned</span>
        <span style={{ color: remaining === 0 ? "var(--accent)" : "var(--negative)" }}>
          {remaining === 0 ? "✓ balanced" : `$${(Math.abs(remaining) / 100).toFixed(2)} ${remaining > 0 ? "left" : "over"}`}
        </span>
      </div>

      {/* Split rows */}
      {rows.map(row => (
        <div key={row.id} style={{ display: "flex", gap: 8, alignItems: "flex-start", marginBottom: 10 }}>
          <select
            value={row.categoryId ?? ""}
            onChange={e => updateRow(row.id, { categoryId: e.target.value || null })}
            style={{ flex: 1, background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 6, padding: "6px 8px", fontSize: 14, color: "var(--ink)" }}
          >
            <option value="">No category</option>
            {categories.map(cat => (
              <option key={cat.id} value={cat.id}>{cat.label}</option>
            ))}
          </select>
          <input
            type="number"
            step="0.01"
            min="0.01"
            placeholder="0.00"
            value={row.amountCents > 0 ? (row.amountCents / 100).toFixed(2) : ""}
            onChange={e => updateRow(row.id, { amountCents: Math.round(parseFloat(e.target.value || "0") * 100) })}
            style={{ width: 90, background: "var(--surface-2)", border: "1px solid var(--line)", borderRadius: 6, padding: "6px 8px", fontSize: 14, color: "var(--ink)" }}
          />
          {rows.length > 2 && (
            <button type="button" onClick={() => removeRow(row.id)} style={{ color: "var(--ink-faint)", fontSize: 18, lineHeight: 1, padding: "4px 6px", background: "none", border: "none", cursor: "pointer" }}>×</button>
          )}
        </div>
      ))}

      <button
        type="button"
        onClick={() => setRows(prev => [...prev, newRow()])}
        style={{ marginBottom: 24, background: "none", border: "1px dashed var(--line)", borderRadius: 6, color: "var(--ink-mute)", padding: "6px 12px", fontSize: 13, cursor: "pointer", width: "100%" }}
      >
        + Add split
      </button>

      <div className="form-actions">
        <button type="button" onClick={onClose}>Cancel</button>
        <button type="button" className="primary" disabled={!balanced || setSplits.isPending} onClick={handleSave}>
          {setSplits.isPending ? "Saving…" : "Save splits"}
        </button>
      </div>
    </Drawer>
  );
}
