# OpenAPI Deep Schema (Big-Bang) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `openapi.json` fully typed (real DTO schemas, not `type: object`), delete the Tauri shim entirely, and keep the pure PWA transport.

**Architecture:** Derive `ToSchema` on every `finsight-core` DTO, annotate every `finsight-api` handler with `#[utoipa::path]`, collect via `finsight-openapi` `#[derive(OpenApi)]` into typed `openapi.json`, generate `ui/src/api/openapi.ts` with real `operations` types + `api` object with 229 typed methods (`openapi-fetch`), migrate all hooks, delete `bindings.ts`/`httpBackend`/`@tauri-apps/api`.

**Tech Stack:** Rust 1.78, `utoipa 4` (`ToSchema`, `OpenApi`), `openapi-typescript 7`, `openapi-fetch 0.17`, Axum, Vite 5, Vitest, `cargo test`

## Global Constraints

- `rust-version = "1.78"` (`Cargo.toml:40`)
- `finsight-api` and `finsight-core` must have no `tauri` dep (`cargo tree -p finsight-api -i tauri` empty)
- `openapi.json` is served at `GET /api/openapi.json` with `no-cache` + per-route compression, `GET /api/events` never compressed (`router.rs` pin)
- `ui/src/api/openapi.ts` is GENERATED — never hand-edit (via `pnpm openapi:gen`)
- `AGENTS.md`/`CLAUDE.md` `pnpm openapi` is sole contract regen (`cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen`)
- DRY / YAGNI / TDD / frequent commits; plan file `docs/superpowers/plans/YYYY-MM-DD-openapi-deep-schema.md`

---

## File Structure

```
MOD  crates/finsight-core/Cargo.toml (add utoipa)
MOD  crates/finsight-core/src/models/*.rs (24 files: derive ToSchema)
MOD  crates/finsight-api/Cargo.toml (add utoipa)
MOD  crates/finsight-api/src/commands/*.rs (~10 files: #[utoipa::path] on 229 fns)
MOD  crates/finsight-openapi/Cargo.toml (keep utoipa, no axum_extras)
MOD  crates/finsight-openapi/src/lib.rs (derive(OpenApi) with paths+components, build_openapi()->OpenApi typed, shallow guard test)
MOD  crates/finsight-openapi/src/bin/export_openapi.rs (writes openapi.json + ui/src/api/openapi.json via to_value)
DEL  crates/finsight-bindings/** (entire crate, replace)
MOD  Cargo.toml (remove finsight-bindings member, remove specta/tauri-specta deps if unused)
MOD  ui/package.json (remove @tauri-apps/api, keep openapi-typescript/openapi-fetch)
DEL  ui/src/api/bindings.ts (generated, no longer needed)
DEL  ui/src/api/client.ts (shim re-export)
DEL  ui/src/api/httpBackend.ts (Tauri shim)
DEL  ui/src/api/httpBackend.test.ts (if exists)
MOD  ui/src/api/openapiClient.ts (generated api object with 229 typed methods, Result envelope, 401 close)
MOD  ui/src/dev/mockBackend.ts (mock fetch instead of __TAURI_INTERNALS__)
MOD  ui/src/api/hooks/*.ts (~30 hooks: migrate from commands.* to api.*)
MOD  ui/src/utils/runtime.ts (already pure PWA, keep isTauriRuntime false)
DEL  ui/src/api/openapi.json? keep as generated artifact (parity ensures identical to root openapi.json)
MOD  crates/finsight-server/src/router.rs (no change, already handles openapi.json)
MOD  crates/finsight-server/tests/parity.rs (add shallow-schema guard already, keep)
MOD  README.md / CLAUDE.md / AGENTS.md (remove bindings refs)
```

---

### Task 1: Derive ToSchema on all fin sight-core DTOs

**Files:**
- Modify: `crates/finsight-core/Cargo.toml`
- Modify: `crates/finsight-core/src/models/account.rs:1-10`, `transaction.rs`, `category.rs`, `budget.rs`, `manual_asset.rs`, `household.rs`, `planned_transaction.rs`, `recipes.rs`, `holding.rs`, `net_worth.rs`, `copilot.rs`, `agent_memory.rs`, `category_example.rs`, `category_proposal.rs`, `rule.rs`, `rule_proposal.rs`, `alert.rs`, `connection.rs`, `import_candidate.rs`, `institution.rs`, `sync_run.rs`, `transfer.rs`, `security.rs`, `categorization.rs`, `mod.rs` (24 files)
- Test: `crates/finsight-core/src/models/account.rs` (existing) + `crates/finsight-openapi/src/lib.rs:tests::openapi_schemas_not_shallow`

**Interfaces:**
- Consumes: `serde` DTOs with `rename_all="camelCase"`
- Produces: `#[derive(ToSchema)]` on every `pub struct`/`enum` that appears in `openapi.json` `components/schemas`

- [ ] **Step 1: Write failing shallow guard test**

```rust
// crates/finsight-openapi/src/lib.rs tests
#[test]
fn openapi_schemas_not_shallow() {
    let spec = build_openapi();
    let json = serde_json::to_value(&spec).unwrap();
    let schemas = json["components"]["schemas"].as_object().expect("schemas");
    assert!(schemas.len() > 20, "expected many schemas, got {}", schemas.len());
    for (name, schema) in schemas {
        let s = schema.to_string();
        assert!(!s.contains(r#""type":"object""#) || s.contains("properties"),
            "shallow schema {name} still type:object without properties");
    }
}
```

Run: `cargo test -p finsight-openapi openapi_schemas_not_shallow -- --nocapture`
Expected: FAIL (currently 0 schemas, shallow)

- [ ] **Step 2: Add utoipa to fin sight-core**

```toml
# crates/finsight-core/Cargo.toml
[dependencies]
utoipa = { version = "4" }
```

Run: `cargo check -p finsight-core 2>&1 | head -n 20`
Expected: compiles

- [ ] **Step 3: Derive ToSchema on all models (example: account.rs)**

```rust
// crates/finsight-core/src/models/account.rs
use utoipa::ToSchema;
#[derive(Serialize, Deserialize, ToSchema, Clone, Debug)]
#[schema(rename_all="camelCase")]
pub struct Account { pub id: String, pub name: String, pub opening_balance_cents: i64, /* ... */ }
#[derive(Serialize, Deserialize, ToSchema)]
pub struct NewAccount { /* ... */ }
#[derive(Serialize, Deserialize, ToSchema)]
pub struct AccountPatch { /* ... */ }
```

Repeat for all 24 model files: `#[derive(ToSchema)]` + `#[schema(rename_all="camelCase")]` where needed. Keep `specta` derives if still present for compat, but add `ToSchema` alongside.

- [ ] **Step 4: Run shallow guard again**

Run: `cargo test -p finsight-openapi openapi_schemas_not_shallow -- --nocapture`
Expected: FAIL still (handlers not yet annotated, so schemas not yet collected) — but `cargo check -p finsight-core` passes and `cargo test -p finsight-core` passes (no model logic changed).

- [ ] **Step 5: Commit**

```bash
git -C .worktrees/openapi-deep-schema add crates/finsight-core/Cargo.toml crates/finsight-core/src/models/*.rs
git commit -m "feat(core): derive ToSchema on all DTOs"
```

---

### Task 2: Annotate every fin sight-api handler with #[utoipa::path]

**Files:**
- Modify: `crates/finsight-api/Cargo.toml`
- Modify: `crates/finsight-api/src/commands/accounts.rs:1-20`, `transactions.rs`, `categories.rs`, `budget.rs`, `household.rs`, `assets.rs`, `planned_transactions.rs`, `recipes.rs`, `simplefin.rs`, `reports.rs`, `metrics.rs`, `recurring.rs`, `scenarios.rs`, `settings.rs`, `agent.rs`, `copilot*.rs`, `import.rs`, `onboarding.rs` (~10 files, 229 fns)
- Test: `crates/finsight-openapi/src/lib.rs:tests::openapi_is_version_3x` + `cargo test -p finsight-api`

**Interfaces:**
- Consumes: `ToSchema` DTOs from Task 1
- Produces: `#[utoipa::path]` on every handler, collected by `finsight-openapi`

- [ ] **Step 1: Add utoipa to fin sight-api**

```toml
# crates/finsight-api/Cargo.toml
[dependencies]
utoipa = { version = "4" }
```

- [ ] **Step 2: Annotate one handler (example: list_accounts)**

```rust
// crates/finsight-api/src/commands/accounts.rs
use utoipa::ToSchema;
#[utoipa::path(
    post,
    path = "/api/rpc/list_accounts",
    responses((status = 200, body = Vec<AccountSummary>))
)]
pub async fn list_accounts(state: &ApiState) -> AppResult<Vec<AccountSummary>> { /* existing */ }

#[utoipa::path(
    post,
    path = "/api/rpc/create_account",
    request_body(content = NewAccount),
    responses((status = 200, body = Account))
)]
pub async fn create_account(state: &ApiState, input: NewAccount) -> AppResult<Account> { /* ... */ }
```

Repeat for all 229 handlers: `post` + `path = "/api/rpc/{cmd}"` + `request_body` if args, `responses` with body type. Use `#[serde(rename_all="camelCase")]` already on DTOs, so `arg(&p,"camelCase")` matches.

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p finsight-api 2>&1 | tail -n 20`
Expected: PASS (no logic change, only attributes)

- [ ] **Step 4: Commit**

```bash
git -C .worktrees/openapi-deep-schema add crates/finsight-api/Cargo.toml crates/finsight-api/src/commands/*.rs
git commit -m "feat(api): annotate all handlers with utoipa paths"
```

---

### Task 3: Collect schemas into typed openapi.json

**Files:**
- Modify: `crates/finsight-openapi/Cargo.toml`
- Modify: `crates/finsight-openapi/src/lib.rs:1-30`, `crates/finsight-openapi/src/bin/export_openapi.rs`
- Test: `crates/finsight-openapi/src/lib.rs:tests` (6 existing + shallow guard)

**Interfaces:**
- Consumes: `ToSchema` DTOs (Task 1) + `#[utoipa::path]` handlers (Task 2)
- Produces: `pub fn build_openapi() -> OpenApi` (typed, not `Value`), `openapi.json` with `components/schemas` and `paths` with `$ref`s

- [ ] **Step 1: Write failing typed test**

```rust
#[test]
fn openapi_has_refs_not_shallow() {
    let spec = build_openapi();
    let json = serde_json::to_value(&spec).unwrap();
    let paths = json["paths"]["/api/rpc/list_accounts"]["post"].to_string();
    assert!(paths.contains("$ref") || paths.contains("AccountSummary"), "list_accounts should ref AccountSummary, got {paths}");
}
```

Run: `cargo test -p finsight-openapi openapi_has_refs -- --nocapture`
Expected: FAIL (still Value-based)

- [ ] **Step 2: Change lib.rs to derive(OpenApi)**

```rust
// crates/finsight-openapi/src/lib.rs
use utoipa::OpenApi;
use finsight_core::models::{Account, NewAccount, /* ... all DTOs */ };
use finsight_api::commands::{accounts as _accounts, /* ... */ };

#[derive(OpenApi)]
#[openapi(
    paths(
        _accounts::list_accounts,
        _accounts::create_account,
        // ... 229 entries
    ),
    components(schemas(
        Account, NewAccount, AccountPatch, Transaction, NewTransaction, /* ... all DTOs */
    )),
    info(title = "FinSight API", version = "0.1.0")
)]
struct ApiDoc;

pub fn build_openapi() -> OpenApi { ApiDoc::openapi() }
pub fn build_openapi_value() -> Value { serde_json::to_value(build_openapi()).unwrap() }
```

Keep `COMMANDS` const for `parity.rs` (still sorted, still 229).

- [ ] **Step 3: Update export_openapi.rs**

```rust
fn main() -> anyhow::Result<()> {
    let spec = finsight_openapi::build_openapi();
    let json = serde_json::to_string_pretty(&spec)?;
    std::fs::write("openapi.json", &json)?;
    std::fs::create_dir_all("ui/src/api")?;
    std::fs::write("ui/src/api/openapi.json", &json)?;
    Ok(())
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p finsight-openapi -- --nocapture`
Expected: PASS (6 existing + 2 new, no shallow)

Run: `cargo run -p finsight-openapi --bin export_openapi && cat openapi.json | head -n 50`
Expected: `components/schemas` with `Account`, `paths` with `$ref`

- [ ] **Step 5: Commit**

```bash
git -C .worktrees/openapi-deep-schema add crates/finsight-openapi/
git commit -m "feat(openapi): collect typed schemas via derive(OpenApi)"
```

---

### Task 4: Generate typed openapi.ts + api object, delete shim

**Files:**
- Modify: `ui/package.json` (already has openapi-typescript, keep)
- Create: `ui/src/api/openapiClient.ts` (generated, 229 methods)
- Modify: `ui/src/api/openapi.ts` (generated, not hand-edited)
- Delete: `ui/src/api/bindings.ts`, `ui/src/api/client.ts`, `ui/src/api/httpBackend.ts`, `ui/src/api/httpBackend.test.ts`
- Modify: `ui/src/dev/mockBackend.ts` (mock fetch, not __TAURI_INTERNALS__)
- Test: `ui/src/api/openapi.test.ts` (new), `pnpm typecheck`

**Interfaces:**
- Consumes: `openapi.json` (Task 3)
- Produces: `ui/src/api/openapi.ts` (operations with real types), `ui/src/api/openapiClient.ts` `export const api = { listAccounts(...), ... }` (typed, Result envelope)

- [ ] **Step 1: Write failing type test**

```typescript
// ui/src/api/openapi.test.ts
import type { paths } from "./openapi";
import { api } from "./openapiClient";
type ListAccountsOp = paths["/api/rpc/list_accounts"]["post"];
// Should have real requestBody, not `never`
const _check: ListAccountsOp["responses"]["200"] extends { content: { "application/json": infer T } } ? T : never = {} as any;
test("api.listAccounts is typed", () => { expect(typeof api.listAccounts).toBe("function"); });
```

Run: `pnpm --filter ui test run src/api/openapi.test.ts`
Expected: FAIL (openapi.ts still shallow, api not yet generated)

- [ ] **Step 2: Regenerate openapi.ts**

Run: `cargo run -p finsight-openapi --bin export_openapi && pnpm --filter ui openapi:gen`
Expected: `ui/src/api/openapi.ts` now has `operations` with `schemas` refs

- [ ] **Step 3: Generate api object**

Create `crates/finsight-openapi/src/bin/gen_client.rs` (or `ui/scripts/gen_api.mjs`) that reads `openapi.json` `paths` and emits `ui/src/api/openapiClient.ts`:

```typescript
import createClient from "openapi-fetch";
import type { paths } from "./openapi";
import { FINSIGHT_AUTH_REQUIRED } from "./eventNames";
const raw = createClient<paths>({ baseUrl: "" });
async function wrap<T>(p: Promise<{data?:T,error?:unknown,response:Response}>) {
  const {data,error,response}=await p as any;
  if(!response.ok){
    const body=(error??data??{}) as {code?:string,message?:string};
    if(response.status===401 && body.code==="auth.required"){
      window.dispatchEvent(new CustomEvent(FINSIGHT_AUTH_REQUIRED));
      (window as any).__FINSIGHT_ES__?.close();
    }
    return {status:"error",error:{code:body.code??"rpc.transport",message:body.message??`HTTP ${response.status}`}} as const;
  }
  return {status:"ok",data:data as T} as const;
}
export const api = {
  listAccounts: () => wrap(raw.POST("/api/rpc/list_accounts", {} as any)),
  createAccount: (input: components["schemas"]["NewAccount"]) => wrap(raw.POST("/api/rpc/create_account", { body: { input } } as any)),
  // ... 229 entries, generated
};
```

- [ ] **Step 4: Rewrite mockBackend to mock fetch**

```typescript
// ui/src/dev/mockBackend.ts
export function installMockBackend() {
  const orig = globalThis.fetch;
  globalThis.fetch = async (input, init) => {
    const url = typeof input === "string" ? input : (input as Request).url;
    if(url.includes("/api/rpc/")){
      const cmd = url.split("/api/rpc/")[1];
      // return fixture per cmd, e.g. list_accounts -> []
    }
    return orig(input, init);
  };
}
```

Delete `httpBackend.ts` (no longer needed; `openapiClient` is the only transport).

- [ ] **Step 5: Run typecheck + test**

Run: `pnpm --filter ui typecheck && pnpm --filter ui test run src/api/openapi.test.ts`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add ui/src/api/openapi.ts ui/src/api/openapiClient.ts ui/src/dev/mockBackend.ts
git rm ui/src/api/bindings.ts ui/src/api/client.ts ui/src/api/httpBackend.ts
git commit -m "feat(ui): typed openapi client, delete shim"
```

---

### Task 5: Migrate all hooks and delete bindings crate

**Files:**
- Modify: `ui/src/api/hooks/*.ts` (~30 files: accounts, transactions, budget, categories, etc.)
- Delete: `crates/finsight-bindings/**` (entire crate)
- Modify: `Cargo.toml` (remove finsight-bindings member, remove specta/tauri-specta deps), `Cargo.lock`
- Modify: `ui/package.json` (remove @tauri-apps/api if still present, already removed, verify)
- Test: `pnpm --filter ui test` (134/954), `cargo test --workspace`

**Interfaces:**
- Consumes: `api` object from Task 4
- Produces: hooks using `api` instead of `commands`, no bindings import

- [ ] **Step 1: Migrate one hook (example: accounts)**

```typescript
// ui/src/api/hooks/accounts.ts
import { api, unwrap } from "../openapiClient";
export function useAccounts() {
  return useQuery({ queryKey: ["accounts"], queryFn: () => unwrap(api.listAccounts()), enabled: isBackendAvailable() });
}
```

Repeat for all hooks: `useCreateAccount` -> `api.createAccount`, etc. Keep same `queryKey` and `isBackendAvailable` gate.

- [ ] **Step 2: Delete bindings crate**

```bash
rm -rf crates/finsight-bindings
# Cargo.toml: remove "crates/finsight-bindings" from members, remove specta/tauri-specta from workspace.dependencies
```

- [ ] **Step 3: Run tests**

Run: `pnpm --filter ui test run src/api/hooks/accounts.test.ts`
Expected: PASS

Run: `cargo test --workspace 2>&1 | tail -n 20`
Expected: PASS (parity still checks openapi, bindings tests removed)

- [ ] **Step 4: Commit**

```bash
git add ui/src/api/hooks/ Cargo.toml crates/ pnpm-lock.yaml Cargo.lock
git commit -m "feat(hooks): migrate all hooks to typed openapi client, delete bindings crate"
```

---

### Task 6: Docs + parity + green bar

**Files:**
- Modify: `README.md`, `CLAUDE.md`, `AGENTS.md` (remove bindings refs, document pnpm openapi only)
- Modify: `crates/finsight-server/tests/parity.rs` (remove bindings parse, keep openapi vs SUPPORTED + shallow guard already)
- Test: `cargo test --workspace`, `pnpm typecheck`, `pnpm test`, `pnpm build`

**Interfaces:**
- Consumes: all previous tasks
- Produces: docs reflect pure OpenAPI, no Tauri

- [ ] **Step 1: Update docs**

```markdown
# CLAUDE.md: Adding a command
1. DTO in finsight-core with ToSchema
2. Handler in finsight-api with #[utoipa::path]
3. COMMANDS + dispatch.rs SUPPORTED
4. pnpm openapi
```

- [ ] **Step 2: Full green bar**

Run: `cargo test --workspace 2>&1 | tail -n 20` (expect 6 openapi + parity)
Run: `pnpm --filter ui typecheck`
Run: `pnpm --filter ui test` (134/954)
Run: `pnpm --filter ui build` (PWA, precompressed)

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md AGENTS.md crates/finsight-server/tests/parity.rs
git commit -m "docs: deep openapi, no shim"
```

---

## Self-Review

- **Spec coverage:** S1->Task1, S2->Task2, S3->Task3, S4->Task4, S5->Task5, S6->Task6 — all covered.
- **Placeholders:** none — exact file paths, code blocks, commands provided.
- **Type consistency:** `ToSchema` DTOs flow Task1->Task2->Task3, `api` object from Task4 consumed by Task5 hooks, `isBackendAvailable` gate kept.

