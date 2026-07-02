import { useState } from "react";
import { commands } from "../../api/client";
import { userErrorMessage } from "../../utils/runtime";
import { DEFAULT_CATEGORY_COLOR, paletteFor } from "../../utils/categoryColor";
import Button from "../../components/Button";
import Input from "../../components/Input";
import Select from "../../components/Select";
import Swatch from "../../components/Swatch";

interface Props { onNext: () => void; }

interface Row { id: string; label: string; group_id: string; color: string; }

const COLOR_CHOICES = [
  "#A78BFA", "#34D399", "#FB923C", "#60A5FA", "#FACC15",
  "#F472B6", "#2DD4BF", "#FCA5A5", "#818CF8", "#FDE68A",
  DEFAULT_CATEGORY_COLOR,
];

const STARTERS: Row[] = [
  { id: "housing",       label: "Housing",       group_id: "fixed",     color: paletteFor("housing") },
  { id: "utilities",     label: "Utilities",     group_id: "fixed",     color: paletteFor("utilities") },
  { id: "subscriptions", label: "Subscriptions", group_id: "fixed",     color: paletteFor("subscriptions") },
  { id: "groceries",     label: "Groceries",     group_id: "daily",     color: paletteFor("groceries") },
  { id: "dining",        label: "Dining",        group_id: "daily",     color: paletteFor("dining") },
  { id: "transport",     label: "Transport",     group_id: "daily",     color: paletteFor("transport") },
  { id: "shopping",      label: "Shopping",      group_id: "lifestyle", color: paletteFor("shopping") },
  { id: "travel",        label: "Travel",        group_id: "lifestyle", color: paletteFor("travel") },
  { id: "gifts",         label: "Gifts",         group_id: "lifestyle", color: paletteFor("gifts") },
  { id: "health",        label: "Health",        group_id: "wellbeing", color: paletteFor("health") },
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
    setRows((r) => [
      ...r,
      { id: crypto.randomUUID(), label: "", group_id: "daily", color: DEFAULT_CATEGORY_COLOR },
    ]);
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
              <div
                className="swatch-row"
                aria-label={`Category ${i + 1} color`}
                role="radiogroup"
              >
                {COLOR_CHOICES.map((c) => (
                  <Swatch
                    key={c}
                    color={c}
                    selected={c === row.color}
                    onClick={() => update(i, { color: c })}
                    label={`Choose ${c}`}
                  />
                ))}
              </div>
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
                <span className="cswatch" style={{ background: row.color }} />
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
