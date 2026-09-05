import { useEffect, useState } from "react";
import { api } from "../../api/openapiClient";
import { unwrap } from "../../api/openapiClient";
import { userErrorMessage } from "../../utils/runtime";
import {
  CATEGORY_ICON_CHOICES,
  DEFAULT_CATEGORY_COLOR,
  iconComponentFor,
  iconIdForCategory,
  nextCategoryColor,
  paletteFor,
  type CategoryIconId,
} from "../../utils/categoryColor";
import Button from "../../components/Button";
import Input from "../../components/Input";
import Select from "../../components/Select";

interface Props { onNext: () => void; }

interface Row { id: string; label: string; group_id: string; color: string; icon: CategoryIconId; }

const STARTERS: Row[] = [
  { id: "housing",       label: "Housing",       group_id: "fixed",     color: paletteFor("housing"),       icon: iconIdForCategory("housing") },
  { id: "utilities",     label: "Utilities",     group_id: "fixed",     color: paletteFor("utilities"),     icon: iconIdForCategory("utilities") },
  { id: "subscriptions", label: "Subscriptions", group_id: "fixed",     color: paletteFor("subscriptions"), icon: iconIdForCategory("subscriptions") },
  { id: "groceries",     label: "Groceries",     group_id: "daily",     color: paletteFor("groceries"),     icon: iconIdForCategory("groceries") },
  { id: "dining",        label: "Dining",        group_id: "daily",     color: paletteFor("dining"),        icon: iconIdForCategory("dining") },
  { id: "transport",     label: "Transport",     group_id: "daily",     color: paletteFor("transport"),     icon: iconIdForCategory("transport") },
  { id: "shopping",      label: "Shopping",      group_id: "lifestyle", color: paletteFor("shopping"),      icon: iconIdForCategory("shopping") },
  { id: "travel",        label: "Travel",        group_id: "lifestyle", color: paletteFor("travel"),        icon: iconIdForCategory("travel") },
  { id: "gifts",         label: "Gifts",         group_id: "lifestyle", color: paletteFor("gifts"),         icon: iconIdForCategory("gifts") },
  { id: "health",        label: "Health",        group_id: "wellbeing", color: paletteFor("health"),        icon: iconIdForCategory("health") },
];

const GROUPS = ["fixed", "daily", "lifestyle", "wellbeing"] as const;
const GROUP_LABELS: Record<(typeof GROUPS)[number], string> = {
  fixed: "Fixed",
  daily: "Daily",
  lifestyle: "Lifestyle",
  wellbeing: "Wellbeing",
};

export default function StepCategories({ onNext }: Props) {
  const [rows, setRows] = useState<Row[]>(STARTERS);
  const [openIconId, setOpenIconId] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    if (!openIconId) return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpenIconId(null);
    };
    const closeOnOutsideClick = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Element && !target.closest(".category-icon-picker")) {
        setOpenIconId(null);
      }
    };
    document.addEventListener("keydown", closeOnEscape);
    document.addEventListener("pointerdown", closeOnOutsideClick);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
      document.removeEventListener("pointerdown", closeOnOutsideClick);
    };
  }, [openIconId]);

  function update(i: number, patch: Partial<Row>) {
    setRows((r) => r.map((row, idx) => (idx === i ? { ...row, ...patch } : row)));
  }
  function add() {
    setRows((r) => [
      ...r,
      // Least-used palette color keeps every added category distinct.
      {
        id: crypto.randomUUID(),
        label: "",
        group_id: "daily",
        color: nextCategoryColor(r.map((row) => row.color)),
        icon: "tag",
      },
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
      await unwrap(api.commitStarterCategories(toSave));
      onNext();
    } catch (err) {
      setSaveError(userErrorMessage(err, "Could not save categories. Check your FinSight server connection and try again."));
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
            <li key={row.id} className="category-row">
              <div className="category-icon-picker">
                <button
                  type="button"
                  className="category-icon-trigger"
                  aria-label={`Choose icon for ${row.label || `category ${i + 1}`}`}
                  aria-expanded={openIconId === row.id}
                  aria-haspopup="true"
                  title="Choose an icon"
                  style={{ color: row.color }}
                  onClick={() => setOpenIconId((current) => current === row.id ? null : row.id)}
                >
                  {(() => {
                    const CategoryIcon = iconComponentFor(row.icon);
                    return <CategoryIcon width={17} height={17} />;
                  })()}
                </button>
                {openIconId === row.id && (
                  <div className="category-icon-menu" role="group" aria-label={`Icons for ${row.label || `category ${i + 1}`}`}>
                    {CATEGORY_ICON_CHOICES.map(({ id, label, Icon }) => (
                      <button
                        key={id}
                        type="button"
                        className={`category-icon-option ${row.icon === id ? "selected" : ""}`}
                        aria-label={`Use ${label} icon`}
                        aria-pressed={row.icon === id}
                        title={label}
                        onClick={() => {
                          update(i, { icon: id });
                          setOpenIconId(null);
                        }}
                      >
                        <Icon width={16} height={16} />
                      </button>
                    ))}
                  </div>
                )}
              </div>
              <Input
                value={row.label}
                onChange={(e) => update(i, { label: e.target.value })}
                aria-label={`Category ${i + 1} label`}
                className="category-label-field"
                placeholder="Category name"
              />
              <Select
                value={row.group_id}
                onChange={(e) => update(i, { group_id: e.target.value })}
                aria-label={`Category ${i + 1} group`}
                className="category-group-field"
              >
                {GROUPS.map((g) => (
                  <option key={g} value={g}>
                    {GROUP_LABELS[g]}
                  </option>
                ))}
              </Select>
              <label className="category-color-control">
                <span className="category-color-preview" style={{ background: row.color }} aria-hidden="true" />
                <input
                  type="color"
                  value={row.color || DEFAULT_CATEGORY_COLOR}
                  onChange={(e) => update(i, { color: e.target.value })}
                  aria-label={`Choose color for ${row.label || `category ${i + 1}`}`}
                  title="Choose a color"
                />
              </label>
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
          <h3 className="h3" style={{ marginBottom: 10 }}>How this will look in the app</h3>
          <div className="stack stack-sm">
            {rows.slice(0, 10).map((row) => (
              <div key={row.id} className="onb-category-preview">
                <span className="onb-category-icon" style={{ color: row.color }} aria-hidden="true">
                  {(() => {
                    const CategoryIcon = iconComponentFor(row.icon);
                    return <CategoryIcon width={15} height={15} />;
                  })()}
                </span>
                <div style={{ minWidth: 0 }}>
                  <div style={{ fontSize: 13.5 }}>{row.label || "Untitled category"}</div>
                  <div className="muted" style={{ fontSize: 11.5 }}>{GROUP_LABELS[row.group_id as (typeof GROUPS)[number]] ?? row.group_id}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
