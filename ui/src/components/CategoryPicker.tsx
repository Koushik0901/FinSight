import React, { useMemo, useState } from "react";
import { useCategories } from "../api/hooks/transactions";
import type { CategoryDto } from "../api/bindings";

interface Props {
  value: string | null;
  onChange: (id: string) => void;
}

function groupCategories(cats: CategoryDto[]): Map<string, CategoryDto[]> {
  const map = new Map<string, CategoryDto[]>();
  for (const cat of cats) {
    const existing = map.get(cat.group_label) ?? [];
    existing.push(cat);
    map.set(cat.group_label, existing);
  }
  return map;
}

export default function CategoryPicker({ value, onChange }: Props) {
  const [search, setSearch] = useState("");
  const { data: categories = [], isLoading } = useCategories();

  const filtered = useMemo(() => {
    const s = search.trim().toLowerCase();
    if (!s) return categories;
    return categories.filter(
      (c) => c.label.toLowerCase().includes(s) || c.group_label.toLowerCase().includes(s)
    );
  }, [categories, search]);

  const grouped = useMemo(() => groupCategories(filtered), [filtered]);

  if (isLoading) {
    return <div className="muted" style={{ padding: "8px 0", fontSize: 13 }}>Loading…</div>;
  }

  return (
    <div className="category-picker">
      <input
        role="searchbox"
        type="text"
        placeholder="Search categories…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="control"
        aria-label="Search categories"
      />
      <div role="listbox" aria-label="Category" className="category-list">
        {Array.from(grouped.entries()).map(([groupLabel, cats]) => (
          <div key={groupLabel} role="group" aria-label={groupLabel} className="category-group">
            <div className="category-group-label">{groupLabel}</div>
            {cats.map((cat) => (
              <button
                key={cat.id}
                type="button"
                role="option"
                aria-selected={cat.id === value}
                onClick={() => onChange(cat.id)}
                className={`category-item${cat.id === value ? " selected" : ""}`}
              >
                <span className="dot" style={{ backgroundColor: cat.color }} />
                {cat.label}
              </button>
            ))}
          </div>
        ))}
        {grouped.size === 0 && (
          <p className="muted" style={{ padding: "8px 0", fontSize: 13 }}>No categories found.</p>
        )}
      </div>
    </div>
  );
}
