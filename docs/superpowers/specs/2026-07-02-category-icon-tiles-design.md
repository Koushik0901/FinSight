# Category Icon Tiles — Design

**Status:** Approved, ready for implementation plan
**Tier:** B (buildable for real, no fabrication) — item 1 of 6, from `docs/superpowers/plans/2026-07-01-design-conformance-deep-audit.md`

## Problem

The Categories screen (`ui/src/screens/Categories.tsx`) renders each category's leading visual as a plain filled-color circle (`cswatch`). The Claude Design mockup renders a 22×22px neutral tile containing a semantic monoline icon, colored to the category's accent color. This is a real, buildable gap — not a fabricated mockup element — since the mockup's own icon source is a static id-keyed JS lookup with a generic fallback, which the app can replicate exactly with no backend changes.

## Research findings

- Design mockup (`components/categories.jsx`, `components/icons.jsx` in Claude Design project `fdbc4798-c6d0-41df-9499-e6ca4294d142`) renders:
  ```jsx
  <span style={{ width: 22, height: 22, borderRadius: 6, background: "var(--surface-2)", color: c.color, display: "grid", placeItems: "center" }}>
    <CatIco />
  </span>
  ```
  where `CatIco = I.catIcon[c.id] || I.Tag` — a hardcoded `id`-keyed lookup to 16px monoline icons (stroke 1.4, `currentColor`).
- Mapping used by the mockup: `housing→house, groceries→cart, dining→fork, transport→car, utilities→bulb, subscriptions→box, health→heart, shopping→tag (generic), travel→plane, gifts→gift`.
- `ui/src/components/Icons.tsx` already has the same `icon()` factory pattern used by the mockup, but only generic/nav icons are ported — none of the 9 semantic category icons above exist yet (Tag already exists and is reused for `shopping` and the fallback).
- `ui/src/utils/categoryColor.ts` already has an equivalent lookup for color, `paletteFor(id)`, keyed by the same lowercase semantic ids (`housing`, `groceries`, `dining`, etc.) used at category-creation time in `StepCategories.tsx`. The icon lookup should mirror this exact convention for consistency — same file, same keying, same fallback pattern.
- The `Category` DB model (`crates/finsight-core/src/models/category.rs`) already has an unused `icon: Option<String>` column (schema since `V001__initial_schema.sql`), but it is not selected by `CategoryWithSpending`/`listCategoriesWithSpending` (the query actually used by the Categories screen), and no picker UI exists in either the app or the mockup. Wiring this column up is a materially bigger lift (query change, bindings regen, a new picker UI) and out of scope for this item — noted below as a possible future enhancement.
- Category ids are only semantic for the ~10 seeded starter categories (`StepCategories.tsx` assigns ids like `"housing"`, `"dining"` at seed time). Categories added later by the user get `crypto.randomUUID()` ids. An id-keyed lookup therefore covers all seeded categories (even if later renamed) but not custom ones — which is the same degradation the mockup itself accepts via its `|| I.Tag` fallback.

## Decision

Client-side, render-time icon lookup keyed by `category.id`, mirroring the existing `paletteFor(id)` color lookup exactly. No backend or schema changes. The already-present-but-unused DB `icon` column is left alone.

## Design

### `ui/src/utils/categoryColor.ts`

Add an `iconFor(id: string): IconComponent` function alongside `paletteFor`, backed by a `Record<string, IconComponent>` keyed by the same 10 seeded ids used by `paletteFor`'s `PALETTE` map. Falls back to the existing `Tag` icon for any id not in the map (covers custom/renamed-with-new-id categories, matching the mockup's own fallback).

```ts
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

(`subs` included to mirror `PALETTE`'s existing `subs` alias.)

### `ui/src/components/Icons.tsx`

Port 9 new semantic icons from the design mockup's `icons.jsx`, copying SVG paths verbatim to preserve the monoline style (16px viewBox, stroke width 1.4, `currentColor`), using the existing `icon()` factory: `House`, `Cart`, `Fork`, `Car`, `Bulb`, `Box`, `Heart`, `Plane`, `Gift`. (`Tag` already exists and is reused for `shopping` and the fallback — no new icon needed for it.)

### `ui/src/screens/Categories.tsx`

The `cswatch` button (currently a filled circle, `background: category.color`, opens the color picker on click) changes its rendered content:

- Wrapper: 22×22px, `border-radius: 6px`, `background: var(--surface-2)`, `display: grid`, `place-items: center` (matches the mockup's tile exactly).
- Content: `iconFor(category.id)` rendered with `color: category.color || DEFAULT_CATEGORY_COLOR` (via the icon component's `currentColor` stroke).
- The button's `onClick` (toggling `openColorId`), `aria-label`, and `aria-expanded` are unchanged — only the visual content inside the button changes. The color-picker swatch row that appears below on click is untouched.

### Testing

Update `ui/src/screens/Categories.test.tsx`:
- Add a `data-testid="cat-icon-{category.id}"` on each rendered icon component's wrapping `svg` (or pass it through the `icon()` factory as an optional prop) so tests can assert on identity without depending on path data. Assert a known seeded category id (e.g. `"groceries"`) renders a `Cart`-identified icon rather than a plain circle.
- Assert an unrecognized custom category id (e.g. a random-looking id not in the seeded list) renders the `Tag`-identified fallback icon.
- Existing color-picker-click tests (open/close, color selection) continue to pass unchanged since the click behavior isn't touched.

Add a unit test to `ui/src/utils/categoryColor.test.ts` mirroring the existing `paletteFor` tests: `iconFor("housing")` returns `House`, `iconFor("unknown")` returns `Tag`, `iconFor("")` returns `Tag`.

## Out of scope

- Wiring the existing DB `icon` column through `CategoryWithSpending`/`listCategoriesWithSpending` and building an icon picker UI, so users could assign a custom icon to any category (including custom ones). Flagged as a possible future enhancement in the audit doc, not needed here since neither the app nor the mockup has a picker today.
- Any change to category creation, renaming, or the color-picker interaction itself.

## Risks / edge cases

- A user who renames a seeded category (e.g. "Groceries" → "Food") keeps its icon correctly, since the lookup is by `id`, not label — this was a specific reason to prefer id-keying over label-keying.
- A user-added custom category always gets the generic `Tag` icon since its id is a random UUID — same limitation the design mockup itself has.
- No migration or bindings regeneration needed; this is a pure frontend change.
