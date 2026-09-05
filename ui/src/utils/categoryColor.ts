/**
 * Mirrors `crates/finsight-core/src/palette.rs`.
 * If you change one side, change the other.
 */
import type { ComponentType, SVGProps } from "react";
import { Box, Bulb, Car, Cart, Fork, Gift, Goal, Heart, House, Plane, Sparkle, Tag, Wallet } from "../components/Icons";

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

export type CategoryIconId =
  | "house"
  | "cart"
  | "fork"
  | "car"
  | "bulb"
  | "box"
  | "heart"
  | "plane"
  | "gift"
  | "tag"
  | "wallet"
  | "goal"
  | "sparkle";

export interface CategoryIconChoice {
  id: CategoryIconId;
  label: string;
  Icon: IconComponent;
}

export const CATEGORY_ICON_CHOICES: readonly CategoryIconChoice[] = [
  { id: "house", label: "Home", Icon: House },
  { id: "cart", label: "Groceries", Icon: Cart },
  { id: "fork", label: "Dining", Icon: Fork },
  { id: "car", label: "Transport", Icon: Car },
  { id: "bulb", label: "Utilities", Icon: Bulb },
  { id: "box", label: "Subscriptions", Icon: Box },
  { id: "heart", label: "Health", Icon: Heart },
  { id: "plane", label: "Travel", Icon: Plane },
  { id: "gift", label: "Gifts", Icon: Gift },
  { id: "tag", label: "Tag", Icon: Tag },
  { id: "wallet", label: "Wallet", Icon: Wallet },
  { id: "goal", label: "Goal", Icon: Goal },
  { id: "sparkle", label: "Other", Icon: Sparkle },
];

const ICON_BY_ID: Record<CategoryIconId, IconComponent> = {
  house: House,
  cart: Cart,
  fork: Fork,
  car: Car,
  bulb: Bulb,
  box: Box,
  heart: Heart,
  plane: Plane,
  gift: Gift,
  tag: Tag,
  wallet: Wallet,
  goal: Goal,
  sparkle: Sparkle,
};

const DEFAULT_ICON_BY_CATEGORY_ID: Record<string, CategoryIconId> = {
  housing: "house",
  groceries: "cart",
  dining: "fork",
  transport: "car",
  utilities: "bulb",
  subscriptions: "box",
  subs: "box",
  health: "heart",
  shopping: "tag",
  travel: "plane",
  gifts: "gift",
};

export function iconIdForCategory(id: string): CategoryIconId {
  return DEFAULT_ICON_BY_CATEGORY_ID[id] ?? "tag";
}

export function iconComponentFor(iconId: string | null | undefined): IconComponent {
  return iconId && iconId in ICON_BY_ID ? ICON_BY_ID[iconId as CategoryIconId] : Tag;
}

export function iconForCategory(iconId: string | null | undefined, categoryId: string): IconComponent {
  return iconId && iconId in ICON_BY_ID ? iconComponentFor(iconId) : iconComponentFor(iconIdForCategory(categoryId));
}

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
  return CATEGORY_ICONS[id] ?? iconComponentFor(iconIdForCategory(id));
}
