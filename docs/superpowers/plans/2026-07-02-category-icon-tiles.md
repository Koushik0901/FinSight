# Category Icon Tiles Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the plain colored-dot category swatch on the Categories screen with a 22×22px icon tile (neutral background, colored semantic monoline icon), matching the Claude Design mockup exactly.

**Architecture:** A new `iconFor(id)` lookup in `ui/src/utils/categoryColor.ts` (mirroring the existing `paletteFor(id)` color lookup, same keys, same fallback pattern) maps the 10 seeded category ids to 9 newly-ported semantic icons in `ui/src/components/Icons.tsx` (`shopping` reuses the existing `Tag` icon, which also serves as the fallback for unrecognized/custom category ids). `Categories.tsx`'s existing color-picker-trigger button swaps its rendered content from a filled circle to this icon inside a tile, with no change to its click behavior.

**Tech Stack:** React + TypeScript, Vitest + `@testing-library/react`, no backend/DB changes.

**Reference:** Design doc at `docs/superpowers/specs/2026-07-02-category-icon-tiles-design.md`.

---

### Task 1: Port semantic category icons and add the `iconFor` lookup

**Files:**
- Modify: `ui/src/components/Icons.tsx`
- Modify: `ui/src/utils/categoryColor.ts`
- Test: `ui/src/utils/categoryColor.test.ts`

- [ ] **Step 1: Write the failing test**

Add this new `describe` block to the end of `ui/src/utils/categoryColor.test.ts` (after the existing `paletteFor` block, same file):

```ts
import { Box, Bulb, Car, Cart, Fork, Gift, Heart, House, Plane, Tag } from "../components/Icons";
import { iconFor } from "./categoryColor";

describe("iconFor", () => {
  it("returns the canonical semantic icon for known starter ids", () => {
    expect(iconFor("housing")).toBe(House);
    expect(iconFor("groceries")).toBe(Cart);
    expect(iconFor("dining")).toBe(Fork);
    expect(iconFor("transport")).toBe(Car);
    expect(iconFor("utilities")).toBe(Bulb);
    expect(iconFor("subscriptions")).toBe(Box);
    expect(iconFor("subs")).toBe(Box);
    expect(iconFor("health")).toBe(Heart);
    expect(iconFor("shopping")).toBe(Tag);
    expect(iconFor("travel")).toBe(Plane);
    expect(iconFor("gifts")).toBe(Gift);
  });

  it("falls back to the generic tag icon for unknown ids", () => {
    expect(iconFor("unknown")).toBe(Tag);
    expect(iconFor("")).toBe(Tag);
  });
});
```

Add the two new imports (`Box, Bulb, Car, Cart, Fork, Gift, Heart, House, Plane, Tag` from `"../components/Icons"`, and `iconFor` from `"./categoryColor"`) to the top of the file, alongside the existing `import { DEFAULT_CATEGORY_COLOR, paletteFor } from "./categoryColor";` line — combine into one import from `./categoryColor`:

```ts
import { describe, expect, it } from "vitest";
import { Box, Bulb, Car, Cart, Fork, Gift, Heart, House, Plane, Tag } from "../components/Icons";
import { DEFAULT_CATEGORY_COLOR, iconFor, paletteFor } from "./categoryColor";
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd ui && npx vitest run src/utils/categoryColor.test.ts`
Expected: FAIL — TypeScript/module error, since `House`, `Cart`, `Fork`, `Car`, `Bulb`, `Box`, `Heart`, `Plane`, `Gift` don't exist yet in `Icons.tsx` and `iconFor` doesn't exist yet in `categoryColor.ts`.

- [ ] **Step 3: Port the 9 missing semantic icons into Icons.tsx**

Append these exports to the end of `ui/src/components/Icons.tsx` (after the existing `Cpu` export), using the same `icon()` factory already defined at the top of the file. Paths are copied verbatim from the Claude Design mockup's `components/icons.jsx` to preserve the exact monoline style:

```tsx
export const House = icon(<path d="M2.5 8 8 3l5.5 5M4 7.5V13h8V7.5" />);

export const Cart = icon(
  <>
    <path d="M2.5 3h2l1 8h7M5.5 8h6.5" />
    <circle cx="6.5" cy="13" r="0.9" />
    <circle cx="11" cy="13" r="0.9" />
  </>
);

export const Fork = icon(
  <>
    <path d="M4 2v12M4 6h3a1 1 0 0 1 1 1v4" />
    <path d="M12 2v12M12 6V2" />
  </>
);

export const Car = icon(
  <>
    <path d="M2.5 11V8l1.5-3h8l1.5 3v3" />
    <path d="M2.5 11h11M3.5 13a1 1 0 1 0 0-2M12.5 13a1 1 0 1 0 0-2" />
  </>
);

export const Bulb = icon(
  <>
    <path d="M5.5 10.5a4 4 0 1 1 5 0V12H5.5z" />
    <path d="M6.5 14h3" />
  </>
);

export const Box = icon(
  <>
    <path d="M2.5 5.5 8 3l5.5 2.5L8 8z" />
    <path d="M2.5 5.5V11L8 13.5V8M13.5 5.5V11L8 13.5" />
  </>
);

export const Heart = icon(
  <path d="M8 13s-5-3.2-5-7a2.5 2.5 0 0 1 5-.5 2.5 2.5 0 0 1 5 .5c0 3.8-5 7-5 7z" />
);

export const Plane = icon(<path d="m2 9 12-5-3 11-3-4.5z" />);

export const Gift = icon(
  <>
    <rect x="2.5" y="6" width="11" height="3" rx="0.6" />
    <rect x="3.5" y="9" width="9" height="5" rx="0.6" />
    <path d="M8 6v8M5.5 6c0-1.5 1-2.5 2.5-2.5S10.5 4.5 10.5 6" />
  </>
);
```

(`Tag` already exists in this file at line 183 and is reused as-is — no change needed to it.)

- [ ] **Step 4: Add the `iconFor` lookup to categoryColor.ts**

Modify `ui/src/utils/categoryColor.ts` to the following full contents:

```ts
/**
 * Mirrors `crates/finsight-core/src/palette.rs`.
 * If you change one side, change the other.
 */
import type { ComponentType, SVGProps } from "react";
import { Box, Bulb, Car, Cart, Fork, Gift, Heart, House, Plane, Tag } from "../components/Icons";

export const DEFAULT_CATEGORY_COLOR = "#94A3B8";

const PALETTE: Record<string, string> = {
  housing:       "#A78BFA",
  groceries:     "#34D399",
  dining:        "#FB923C",
  transport:     "#60A5FA",
  utilities:     "#FACC15",
  subscriptions: "#F472B6",
  subs:          "#F472B6",
  health:        "#2DD4BF",
  shopping:      "#FCA5A5",
  travel:        "#818CF8",
  gifts:         "#FDE68A",
};

export function paletteFor(id: string): string {
  return PALETTE[id] ?? DEFAULT_CATEGORY_COLOR;
}

type IconComponent = ComponentType<SVGProps<SVGSVGElement>>;

const CATEGORY_ICONS: Record<string, IconComponent> = {
  housing: House,
  groceries: Cart,
  dining: Fork,
  transport: Car,
  utilities: Bulb,
  subscriptions: Box,
  subs: Box,
  health: Heart,
  shopping: Tag,
  travel: Plane,
  gifts: Gift,
};

export function iconFor(id: string): IconComponent {
  return CATEGORY_ICONS[id] ?? Tag;
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cd ui && npx vitest run src/utils/categoryColor.test.ts`
Expected: PASS — all tests in the file (both the pre-existing `paletteFor` tests and the new `iconFor` tests) green.

- [ ] **Step 6: Type-check**

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
git add ui/src/components/Icons.tsx ui/src/utils/categoryColor.ts ui/src/utils/categoryColor.test.ts
git commit -m "feat: add semantic category icon lookup (iconFor)"
```

---

### Task 2: Add the `.cat-icon-tile` CSS class

**Files:**
- Modify: `ui/src/styles/app.css`

- [ ] **Step 1: Add the CSS rule**

In `ui/src/styles/app.css`, find the existing `/* ── Category swatch ── */` section (currently at line 784, containing the shared `.cswatch, .swatch` rule used as a plain colored dot in many other screens — Accounts, Budget, Recurring, Reports, Today, etc.). Do **not** modify the existing `.cswatch, .swatch` rule, since it's shared by those other usages. Add a new, separate rule immediately after the `.swatch` block:

```css
.cat-icon-tile {
  width: 22px;
  height: 22px;
  border-radius: 6px;
  background: var(--surface-2);
  display: grid;
  place-items: center;
  border: none;
  padding: 0;
  flex-shrink: 0;
}
```

The full updated section should read:

```css
/* ── Category swatch ──────────────────────────────────── */

.cswatch, .swatch {
  width: 10px;
  height: 10px;
  border-radius: var(--radius-pill);
  display: inline-block;
  flex-shrink: 0;
}
.swatch {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  vertical-align: middle;
  margin-right: 8px;
}
.cat-icon-tile {
  width: 22px;
  height: 22px;
  border-radius: 6px;
  background: var(--surface-2);
  display: grid;
  place-items: center;
  border: none;
  padding: 0;
  flex-shrink: 0;
}
```

- [ ] **Step 2: Commit**

```bash
git add ui/src/styles/app.css
git commit -m "style: add cat-icon-tile class for category icon tiles"
```

(No automated test for a pure CSS addition — it's exercised visually in Task 3 and confirmed working end-to-end there.)

---

### Task 3: Wire the icon tile into the Categories screen

**Files:**
- Modify: `ui/src/screens/Categories.tsx`
- Modify: `ui/src/screens/Categories.test.tsx`

- [ ] **Step 1: Write the failing tests**

In `ui/src/screens/Categories.test.tsx`, change the mock category id `"c1"` to `"groceries"` (a known seeded id) so the icon-tile test can assert the correct semantic icon renders; leave `"c2"` as-is (an id not present in `CATEGORY_ICONS`, so it exercises the fallback path). Update the `useCategoriesWithSpending` mock at the top of the file:

```ts
vi.mock("../api/hooks/transactions", () => ({
  useCategoriesWithSpending: vi.fn(() => ({
    data: [
      { id: "groceries", label: "Groceries", color: "#4ade80", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 30000, lastMonthCents: 50000, txnCount: 5, yearTotalCents: 300000, yearTxnCount: 42, budgetCents: 40000 },
      { id: "c2", label: "Dining Out", color: "#fb923c", groupId: "g1", groupLabel: "Food",
        thisMonthCents: 20000, lastMonthCents: 10000, txnCount: 3, yearTotalCents: 150000, yearTxnCount: 27, budgetCents: 0 },
    ],
    isLoading: false,
    error: null,
  })),
  useSetCategorySpendingType: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
  useUpdateCategoryColor: vi.fn(() => ({ mutateAsync: vi.fn().mockResolvedValue(undefined) })),
}));
```

Add this new `describe` block to the end of the file:

```ts
describe("Categories — icon tiles", () => {
  it("renders the semantic icon for a known seeded category id", () => {
    render(<Categories />, { wrapper: createWrapper() });
    const groceriesIcon = screen.getByTestId("cat-icon-groceries");
    // Cart icon's distinguishing path data (see ui/src/components/Icons.tsx `Cart`)
    expect(groceriesIcon.innerHTML).toContain("M2.5 3h2l1 8h7");
  });

  it("falls back to the generic tag icon for a category id with no semantic match", () => {
    render(<Categories />, { wrapper: createWrapper() });
    const diningIcon = screen.getByTestId("cat-icon-c2");
    // Tag icon's distinguishing path data (see ui/src/components/Icons.tsx `Tag`)
    expect(diningIcon.innerHTML).toContain("M3 3h5.5L13 7.5 8.5 12 4 7.5z");
  });
});
```

- [ ] **Step 2: Run the tests to verify the new ones fail**

Run: `cd ui && npx vitest run src/screens/Categories.test.tsx`
Expected: the pre-existing tests still pass (id rename doesn't affect them — nothing else in the file references `"c1"` by literal string), but the two new tests in `"Categories — icon tiles"` FAIL with "Unable to find an element by: [data-testid=...]", since `Categories.tsx` hasn't been changed yet.

- [ ] **Step 3: Update Categories.tsx**

In `ui/src/screens/Categories.tsx`, change the import line:

```ts
import { DEFAULT_CATEGORY_COLOR } from "../utils/categoryColor";
```

to:

```ts
import { DEFAULT_CATEGORY_COLOR, iconFor } from "../utils/categoryColor";
```

Then, inside the `sorted.map((category) => { ... })` callback (where `current`, `compare`, `budget`, `pct`, `over`, `colorPickerOpen` are already computed), add one more line and change the button's content. Find this block:

```tsx
                const colorPickerOpen = openColorId === category.id;
                return (
                  <tr key={category.id}>
                    <td>
                      <div className="row row-sm">
                        <button
                          type="button"
                          className="cswatch"
                          style={{ background: category.color || "var(--accent)", cursor: "pointer" }}
                          onClick={() => setOpenColorId(colorPickerOpen ? null : category.id)}
                          aria-label={`Change color for ${category.label}`}
                          aria-expanded={colorPickerOpen}
                        />
                        <span>{category.label}</span>
                      </div>
```

Replace it with:

```tsx
                const colorPickerOpen = openColorId === category.id;
                const CategoryIcon = iconFor(category.id);
                return (
                  <tr key={category.id}>
                    <td>
                      <div className="row row-sm">
                        <button
                          type="button"
                          className="cat-icon-tile"
                          style={{ color: category.color || DEFAULT_CATEGORY_COLOR, cursor: "pointer" }}
                          onClick={() => setOpenColorId(colorPickerOpen ? null : category.id)}
                          aria-label={`Change color for ${category.label}`}
                          aria-expanded={colorPickerOpen}
                        >
                          <CategoryIcon data-testid={`cat-icon-${category.id}`} />
                        </button>
                        <span>{category.label}</span>
                      </div>
```

No other lines in the file change — the color-picker swatch row below (`colorPickerOpen && (...)`) is untouched.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd ui && npx vitest run src/screens/Categories.test.tsx`
Expected: PASS — all tests in the file, including the two new icon-tile tests.

- [ ] **Step 5: Run the full frontend suite and type-check**

Run: `cd ui && npx vitest run`
Expected: all tests pass (208 pre-existing + 2 new `iconFor` tests in Task 1 + 2 new icon-tile tests here = 212 total).

Run: `cd ui && npx tsc --noEmit`
Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add ui/src/screens/Categories.tsx ui/src/screens/Categories.test.tsx
git commit -m "feat: replace category color dot with semantic icon tile"
```

---

## Final verification (done by the orchestrating session, not a subagent task)

After all three tasks are committed:
1. Run `cd ui && npx vitest run` and `cd ui && npx tsc --noEmit` one more time from a clean state to confirm no cross-task regressions.
2. Start the app (`pnpm tauri:dev` or `cd ui && npm run dev`) and visually confirm on the Categories screen: each seeded category (Housing, Groceries, Dining, Transport, Utilities, Subscriptions, Health, Shopping, Travel, Gifts) shows its correct semantic icon in a neutral tile colored to match; clicking the tile still opens the color picker as before; picking a new color still updates the icon's color (since it reads `category.color` live).
3. Update the Tier B section of `docs/superpowers/plans/2026-07-01-design-conformance-deep-audit.md` to mark the "Categories per-category icon tiles" item as done, with a one-line pointer to this plan and spec.
