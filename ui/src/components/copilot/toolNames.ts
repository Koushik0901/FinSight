/** Turns a snake_case tool/action kind (e.g. `draft_recategorization`) into a
 *  human-readable label (e.g. `recategorization`). Shared by the tool cards and
 *  the generic tool fallback so both stay in sync. */
export function humanizeToolName(name: string) {
  return name
    .replace(/^draft_/, "")
    .replace(/^get_/, "")
    .replace(/^run_/, "")
    .replaceAll("_", " ");
}
