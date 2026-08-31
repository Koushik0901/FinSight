# Changelog

FinSight does not yet publish versioned GitHub Releases with changelog entries. Until it does, this page tracks notable delivery.

## Unreleased

- **This docs site** — VitePress under `docs/`, GitHub Pages workflow, `pnpm docs:*` scripts.
- Self-hosting guide at `docs/self-hosting.md` remains authoritative for Docker/Tailscale/Caddy/LAN recipes.

## How to track changes

- **Git log**: `git log --oneline --since="1 month ago"`
- **Migrations**: `crates/finsight-core/migrations/V016*` is the latest; next is `V017__`.
- **OpenAPI**: `openapi.json` diff after `pnpm openapi` shows typed contract changes.
- **Server image**: `ghcr.io/koushik0901/finsight:latest` — pin `FINSIGHT_IMAGE` in `.env` for reproducible deploys.

When versioned Releases ship, this page will read from `CHANGELOG.md` and the Release notes. Until then, treat the commit history and the audit files in `docs/audits/` as the change record.

> This page intentionally makes no claims about release cadence or version numbers that do not yet exist.
