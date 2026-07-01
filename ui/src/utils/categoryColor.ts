/**
 * Mirrors `crates/finsight-core/src/palette.rs`.
 * If you change one side, change the other.
 */
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
