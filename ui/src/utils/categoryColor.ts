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

/**
 * The canonical picker choices, in palette order. Single source for every
 * color-picking surface (Categories screen, onboarding) so a category keeps
 * the same hue everywhere.
 */
export const CATEGORY_COLOR_CHOICES: string[] = [
  "#A78BFA", "#34D399", "#FB923C", "#60A5FA", "#FACC15",
  "#F472B6", "#2DD4BF", "#FCA5A5", "#818CF8", "#FDE68A",
];

/**
 * Deterministically pick a color for a NEW category: the least-used palette
 * choice given the colors already in use (first in palette order on ties).
 * Keeps new categories visually distinct instead of stamping them all grey.
 */
export function nextCategoryColor(colorsInUse: readonly (string | null | undefined)[]): string {
  const counts = new Map<string, number>(CATEGORY_COLOR_CHOICES.map((c) => [c, 0]));
  for (const raw of colorsInUse) {
    const c = raw?.toUpperCase();
    if (c && counts.has(c)) counts.set(c, (counts.get(c) ?? 0) + 1);
  }
  let best = CATEGORY_COLOR_CHOICES[0]!;
  let bestCount = Number.POSITIVE_INFINITY;
  for (const c of CATEGORY_COLOR_CHOICES) {
    const n = counts.get(c) ?? 0;
    if (n < bestCount) {
      best = c;
      bestCount = n;
    }
  }
  return best;
}

/**
 * Best-effort color for a category *label* ("Dining", "groceries"). The
 * starter labels map 1:1 to palette ids, so Copilot charts that only carry
 * display labels can still color-match the rest of the app. Unknown labels
 * return null so callers can fall back to their own accent.
 */
export function colorForCategoryLabel(label: string): string | null {
  const id = label.trim().toLowerCase();
  return PALETTE[id] ?? null;
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
