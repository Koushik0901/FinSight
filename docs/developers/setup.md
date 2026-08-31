# Development Setup

## Prerequisites

- Rust stable (clippy + rustfmt)
- Node 22 + pnpm 9
- Docker (for production parity tests, optional for local dev)

## Install

From the repository root:

```bash
pnpm install
```

## Run (server mode with hot reload)

Two terminals:

```bash
# Terminal 1: Axum server on http://localhost:8674, data in ./data
cargo run -p finsight-server

# Terminal 2: Vite on http://localhost:5173 — /api proxies to :8674
pnpm dev
```

Or use the server alone — it serves `ui/dist` at `/}` after `pnpm --filter ui build`.

## Environment

| Variable | Dev default | Purpose |
|---|---|---|
| `FINSIGHT_DATA_DIR` | `./data` | Root for `users.db` and per-user dirs |
| `FINSIGHT_UI_DIR` | `/app/ui/dist` in Docker | SPA assets served by server |
| `FINSIGHT_PORT` | `8674` | HTTP listen port |
| `FINSIGHT_COOKIE_SECURE` | `1` | `Secure` on cookies; set `0` for bare-HTTP LAN tests |
| `RUST_LOG` | `info` | Server filter |
| `FINSIGHT_PUBLIC_ORIGIN` | inferred | External origin when proxy headers insufficient |

## Validation

```bash
pnpm typecheck
pnpm --filter ui test
cargo test --workspace
pnpm build
# After changing the API shape:
pnpm openapi   # export_openapi + openapi:gen
```

`pnpm openapi` runs:

```bash
cargo run -p finsight-openapi --bin export_openapi
pnpm --filter ui openapi:gen
```

and `cargo test -p finsight-server --test parity` + `cargo test -p finsight-openapi` assert the spec stays in sync with `SUPPORTED`.

See also: [Testing](/developers/testing), [API & RPC](/developers/api).
