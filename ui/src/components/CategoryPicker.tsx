import React, { useState } from "react";
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

  const filtered = search.trim()
    ? categories.filter(
        (c) =>
          c.label.toLowerCase().includes(search.toLowerCase()) ||
          c.group_label.toLowerCase().includes(search.toLowerCase())
      )
    : categories;

  const grouped = groupCategories(filtered);

  if (isLoading) {
    return <div className="p-2 text-sm text-muted-foreground">Loading…</div>;
  }

  return (
    <div className="flex flex-col gap-1">
      <input
        role="searchbox"
        type="text"
        placeholder="Search categories…"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        className="mb-2 rounded border px-2 py-1 text-sm focus:outline-none focus:ring-1 focus:ring-ring"
      />
      <div role="listbox" className="flex flex-col gap-2">
        {Array.from(grouped.entries()).map(([groupLabel, cats]) => (
          <div key={groupLabel}>
            <div className="mb-1 px-1 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
              {groupLabel}
            </div>
            {cats.map((cat) => (
              <button
                key={cat.id}
                role="option"
                aria-selected={cat.id === value}
                onClick={() => onChange(cat.id)}
                className="flex w-full items-center gap-2 rounded px-2 py-1 text-sm hover:bg-accent aria-selected:bg-accent aria-selected:font-medium"
              >
                <span
                  className="inline-block h-2.5 w-2.5 rounded-full"
                  style={{ backgroundColor: cat.color }}
                />
                {cat.label}
              </button>
            ))}
          </div>
        ))}
        {grouped.size === 0 && (
          <p className="px-2 py-1 text-sm text-muted-foreground">No categories found.</p>
        )}
      </div>
    </div>
  );
}
