# Frontend

React 18 + TypeScript + Vite, TanStack Query, React Router, and a server-served SPA. No Tauri shim.

## Entry

`ui/index.html` bootstraps to `src/main.tsx` → `src/App.tsx`. The HTML defaults to `data-theme="dark" data-density="cozy"` so tokens exist before React mounts and auth/setup screens have correct inputs even before `ThemeProvider`.

Relevant `<meta>`:

- `theme-color: #0A0F02` (lime-adjacent brand)
- `viewport-fit=cover` for PWA

## Data layer

- `api/openapi.ts` — generated types.
- `api/openapiClient.ts` — `createClient` from `openapi-fetch`, envelope unwrapping, 401 → auth gate.
- `api/hooks/` — thin `useQuery`/`useMutation` wrappers, key factory in `_factory.ts`.
- `api/prefetch.ts` — warms summary queries on link hover/focus (`prefetchRoute`).
- `api/invalidation.ts` — maps mutation → query invalidation.
- `pwa/persist.ts` — seven-day IndexedDB persist for queries (`@tanstack/query-async-storage-persister` + `idb-keyval`), encrypted when the PWA has a secure context.
- `utils/runtime.ts` — `isBackendAvailable()` guard.

## Components & screens

| Path | Notes |
|---|---|
| `components/Drawer.tsx` | Slide-in panel primitive with focus lock |
| `components/CommandPalette.tsx` | ⌘K palette |
| `components/copilot/` | Streamdown renderers for rich Copilot cards |
| `components/mobile/*` | Bottom nav + responsive shells |
| `screens/` | One file per route (see `routes.ts`) |
| `routes.ts` | Canonical `APP_ROUTES`; mirrored in Rust `routes.rs` |

Routes and backend link generation are kept honest by `routes.test.ts` — the test enforces that `APP_ROUTES` matches `<Route>` elements and Rust `APP_ROUTES`.

## Styling

See [CSS & Design](/developers/css). Key points:

- Tokens in `styles/tokens.css`, utilities in `styles/app.css`.
- Money figures use class `"money"` for privacy blur.
- Motion respects `prefers-reduced-motion`.

Next: [Rust Crates](/developers/crates).
