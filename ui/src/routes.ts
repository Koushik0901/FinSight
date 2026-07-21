/**
 * Mirror of `crates/finsight-core/src/routes.rs`.
 *
 * The backend hands the UI paths to navigate to — Inbox action items, Copilot
 * post-execution CTAs, missing-data prompts. Rust validates those paths
 * against its own copy of this list so a model can never produce a link to a
 * screen that does not exist.
 *
 * Two things keep the copies honest, both enforced by `routes.test.ts`:
 *  - this list must match the `<Route>` elements declared in `App.tsx`
 *  - this list must match `APP_ROUTES` in `routes.rs`
 *
 * Adding a screen therefore means touching `App.tsx`, this file, and
 * `routes.rs` — the test tells you which one you forgot.
 */
export const APP_ROUTES = [
  "/",
  "/inbox",
  "/import-review",
  "/insights",
  "/accounts",
  "/transactions",
  "/budget",
  "/categories",
  "/recurring",
  "/goals",
  "/journey",
  "/scenarios",
  "/cashflow",
  "/reports",
  "/path-back",
  "/rules",
  "/settings",
  "/settings/users",
  "/copilot",
  "/recipes",
] as const;

/**
 * Routes deliberately kept out of `APP_ROUTES` because the backend must never
 * link to them: the parameterised ledger route (matched structurally in Rust),
 * a developer spike, and a dev-only preview screen.
 */
export const NON_LINKABLE_ROUTES = [
  "/accounts/:id/transactions",
  "/copilot/ag-ui-spike",
  "/dev/genui-preview",
] as const;

export type AppRoutePath = (typeof APP_ROUTES)[number];
