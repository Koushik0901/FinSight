# Contributing

## Ground rules

- **Design tokens first** — see [CSS & Design](/developers/css).
- **Typed API** — do not add raw fetches; extend the typed RPC surface and regenerate the contract.
- **Tests** — Rust + frontend tests must stay green. See [Testing](/developers/testing).
- **No second source of truth** — routes, command lists, and schemas each have one canonical file and a parity test.

## Adding a command

1. Body as `pub async fn my_cmd(state: &ApiState, ...)` in `crates/finsight-api/src/commands/`.
2. `#[utoipa::path]` + `ToSchema` on handler/DTOs; add to `finsight-openapi::COMMANDS` (sorted, identical to `dispatch.rs::SUPPORTED`).
3. Dispatcher arm in `crates/finsight-server/src/dispatch.rs` via `rpc_routes!(…)` using `arg(&p, "camelCase")`.
4. `pnpm openapi` + `cargo test -p finsight-server --test parity` + `cargo test -p finsight-openapi`.
5. `pnpm typecheck` — fix hooks for any DTO shape change.

## Adding a screen

1. File in `ui/src/screens/`, route in `ui/src/App.tsx`.
2. Path in `ui/src/routes.ts::APP_ROUTES` and `crates/finsight-core/src/routes.rs::APP_ROUTES` (mirrored).
3. `routes.test.ts` enforces all three copies match — the test tells you which you forgot.

## Migrations

Add SQL files to `crates/finsight-core/migrations/` named `V00N__description.sql`. Discovered automatically by Refinery via `embed_migrations!`. Current latest is `V016`, next is `V017__description.sql`.

## Docs

Docs live in `docs/` (VitePress). Adding a page is a Markdown file plus one navigation entry in `docs/.vitepress/config.ts` — no component required. See `"docs:dev"` / `"docs:build"` scripts.

## PR checklist

- [ ] `cargo fmt` + `cargo clippy -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `pnpm --filter ui typecheck` + `pnpm --filter ui test`
- [ ] `pnpm openapi` regenerated if you touched the API
- [ ] `pnpm docs:build` passes (no broken links)

See also: [Design Conventions](/developers/css), [API & RPC](/developers/api).
