# Testing

## Rust

```bash
cargo test --workspace
# Single test
cargo test -p finsight-core --lib repos::transactions::tests::update_transaction_notes
```

- `keychain::tests::*` are `#[cfg_attr(target_os = "linux", ignore)]` (gnome-keyring unavailable headless); run on macOS/Windows or ignore.
- `set_key_round_trip` is intermittently flaky under parallel execution on Windows (pre-existing).
- Parity: `cargo test -p finsight-server --test parity` + `cargo test -p finsight-openapi` must pass after any API shape change.

Expected green: 548 Rust tests (+12 ignored live-DB/keychain).

## Frontend

```bash
pnpm --filter ui test        # vitest run
pnpm --filter ui test -- src/screens/Settings.test.tsx
pnpm --filter ui typecheck
```

Setup: `ui/src/test/setup.ts` (jsdom + `@testing-library/react` + axe). axe warnings about canvas in stderr are expected.

Expected green: 436 frontend tests, 0 type errors.

## Lint

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
pnpm --filter ui lint
```

## OpenAPI health

After any command change:

```bash
pnpm openapi
cargo test -p finsight-server --test parity
cargo test -p finsight-openapi
```

See also: [Development Setup](/developers/setup).
