import { useState } from "react";
import { commands } from "../../api/client";
import { userErrorMessage } from "../../utils/runtime";
import Button from "../../components/Button";
import Input from "../../components/Input";
import Select from "../../components/Select";

interface Props { onNext: () => void; }

interface Row { id: string; label: string; group_id: string; }

const STARTERS: Row[] = [
  { id: "housing",       label: "Housing",       group_id: "fixed" },
  { id: "utilities",     label: "Utilities",     group_id: "fixed" },
  { id: "subscriptions", label: "Subscriptions", group_id: "fixed" },
  { id: "groceries",     label: "Groceries",     group_id: "daily" },
  { id: "dining",        label: "Dining",        group_id: "daily" },
  { id: "transport",     label: "Transport",     group_id: "daily" },
  { id: "shopping",      label: "Shopping",      group_id: "lifestyle" },
  { id: "travel",        label: "Travel",        group_id: "lifestyle" },
  { id: "gifts",         label: "Gifts",         group_id: "lifestyle" },
  { id: "health",        label: "Health",        group_id: "wellbeing" },
];

const GROUPS = ["fixed", "daily", "lifestyle", "wellbeing"] as const;

export default function StepCategories({ onNext }: Props) {
  const [rows, setRows] = useState<Row[]>(STARTERS);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  function update(i: number, patch: Partial<Row>) {
    setRows((r) => r.map((row, idx) => (idx === i ? { ...row, ...patch } : row)));
  }
  function add() {
    setRows((r) => [...r, { id: crypto.randomUUID(), label: "", group_id: "daily" }]);
  }
  function remove(i: number) {
    setRows((r) => r.filter((_, idx) => idx !== i));
  }

  async function commit() {
    setSaving(true);
    setSaveError(null);
    try {
      const toSave = rows.filter((r) => r.label.trim().length > 0);
      const result = await commands.commitStarterCategories(toSave);
      if (result.status === "error") throw new Error(result.error.message);
      onNext();
    } catch (err) {
      setSaveError(userErrorMessage(err, "Could not save categories. Try again from the desktop app."));
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="step-categories onb-split">
      <div className="onb-left">
        <div className="num-step">003 · Categories</div>
        <h1>Confirm your starter categories.</h1>
        <p className="lead">Edit or delete anything that does not fit. We only store what you keep.</p>
        <ul className="category-list">
          {rows.map((row, i) => (
            <li key={row.id} className="row-md" style={{ alignItems: "center" }}>
              <Input
                value={row.label}
                onChange={(e) => update(i, { label: e.target.value })}
                aria-label={`Category ${i + 1} label`}
                style={{ marginBottom: 0 }}
              />
              <Select
                value={row.group_id}
                onChange={(e) => update(i, { group_id: e.target.value })}
                aria-label={`Category ${i + 1} group`}
                style={{ marginBottom: 0 }}
              >
                {GROUPS.map((g) => (
                  <option key={g} value={g}>
                    {g}
                  </option>
                ))}
              </Select>
              <Button
                variant="ghost"
                size="sm"
                onClick={() => remove(i)}
                aria-label={`Remove ${row.label || "row"}`}
              >
                ×
              </Button>
            </li>
          ))}
        </ul>
        <div className="onb-actions">
          <Button variant="default" onClick={add}>+ Add category</Button>
          <Button variant="primary" onClick={commit} disabled={saving} loading={saving}>
            {saving ? "Saving…" : "Use these →"}
          </Button>
        </div>
        {saveError && (
          <p role="alert" className="err">
            {saveError}
          </p>
        )}
      </div>

      <div className="onb-right">
        <div className="card">
          <div className="eyebrow"><span className="dot" />Preview</div>
          <div className="h3" style={{ marginBottom: 10 }}>How this will look in the app</div>
          <div className="stack stack-sm">
            {rows.slice(0, 10).map((row) => (
              <div key={row.id} className="onb-category-preview">
                <span className="cswatch" style={{ background: "var(--accent)" }} />
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontSize: 13.5 }}>{row.label || "Untitled category"}</div>
                  <div className="muted" style={{ fontSize: 11.5, textTransform: "capitalize" }}>{row.group_id}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
