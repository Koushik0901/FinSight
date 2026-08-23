import { useEffect } from "react";

/**
 * Shared drawer-seed effect replacing 5× duplicated `eslint-disable` copies in
 * AccountDrawer / GoalDrawer / PlannedTransactionDrawer / AssetDrawer /
 * TransactionDrawer.
 *
 * Runs `seed` whenever `open` or the entity `key` changes (open/close or
 * switching the edited entity). The `seed` closure is intentionally not a
 * dependency — callers build it inline with `reset`/`setState` captures, and
 * adding it would fire on every render. One suppression here kills five copies.
 */
export function useDrawerSeed(
  open: boolean,
  key: string | undefined,
  seed: () => void,
): void {
  useEffect(() => {
    seed();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, open]);
}
