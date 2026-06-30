import type { TxnFilterInput } from "../api/client";

interface TransactionFilterProps {
  value: TxnFilterInput;
  onChange: (filter: TxnFilterInput) => void;
  counts?: { review: number; anomalies: number };
  className?: string;
}

const PRESETS: { label: string; key: "all" | "needs_review" | "anomalies"; value: string | null }[] = [
  { label: "All", key: "all", value: null },
  { label: "Needs review", key: "needs_review", value: "needs_review" },
  { label: "Anomalies", key: "anomalies", value: "anomalies" },
];

export default function TransactionFilter({ value, onChange, counts, className }: TransactionFilterProps) {
  const activePreset = value.filterPreset ?? "all";

  const update = (patch: Partial<TxnFilterInput>) => {
    onChange({ ...value, ...patch });
  };

  return (
    <div className={className} style={{ display: "flex", gap: 10, alignItems: "center", flexWrap: "wrap" }}>
      <div style={{ flex: 1, display: "flex", alignItems: "center", gap: 10, padding: "8px 14px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10, minWidth: 260 }}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="var(--ink-faint)" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>
        <input
          type="search"
          value={value.search ?? ""}
          onChange={(e) => update({ search: e.target.value || null })}
          placeholder="Search by merchant, note, amount, or category…"
          aria-label="Search transactions"
          style={{ flex: 1, background: "transparent", border: 0, outline: 0, fontSize: 13.5, color: "var(--ink)" }}
        />
      </div>
      <input
        type="date"
        aria-label="Start date"
        value={value.startDate ?? ""}
        onChange={(e) => update({ startDate: e.target.value || null })}
        style={{ padding: "8px 10px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10, color: "var(--ink)", fontSize: 13 }}
      />
      <input
        type="date"
        aria-label="End date"
        value={value.endDate ?? ""}
        onChange={(e) => update({ endDate: e.target.value || null })}
        style={{ padding: "8px 10px", background: "var(--surface)", border: "1px solid var(--line)", borderRadius: 10, color: "var(--ink)", fontSize: 13 }}
      />
      <div className="toolbar">
        {PRESETS.map((preset) => {
          const count = preset.key === "needs_review" ? counts?.review : preset.key === "anomalies" ? counts?.anomalies : undefined;
          return (
            <button
              key={preset.key}
              className={activePreset === preset.value ? "on" : ""}
              type="button"
              onClick={() => update({ filterPreset: preset.value })}
            >
              {preset.label} {count ? count : ""}
            </button>
          );
        })}
      </div>
    </div>
  );
}
