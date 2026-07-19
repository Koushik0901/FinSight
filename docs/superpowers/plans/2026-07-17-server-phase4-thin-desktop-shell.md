# FinSight Server — Phase 4: Thin Desktop Shell Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the full local Tauri desktop app with a thin webview shell (Immich-desktop-style): on first launch it asks for a self-hosted FinSight server URL, then navigates its window to that server — from that point on it behaves exactly like the browser/PWA (same `ui/dist` bundle, same HTTP/SSE shim, same auth). The shipped desktop binary keeps zero local command surface and zero local database. Along the way, close a real functional gap the Phase 3 survey found: the 5 CSV/JSON export commands have 501'd since Phase 1 despite a code comment promising a Phase-3 web flow that was never built — every client (browser, PWA, and this new shell) needs them working.

**Architecture:** `crates/finsight-app` stops being a *shipped* app and becomes a **codegen-only** crate: its ~199 `#[tauri::command]` wrappers keep existing so `tauri-specta` can still generate `ui/src/api/bindings.ts` via the `export_bindings` binary, but nothing links them into a running app anymore. The actual shipped binary (`src-tauri`'s `finsight` bin, built from a rewritten `main.rs`) drops its dependency on `finsight_app::configure_app()` entirely and instead: reads a server URL from the OS keychain (reusing `finsight_core::keychain`, already generic); if unset, shows a small bundled "Connect to your server" screen (a normal React screen inside the *existing* `ui/dist` bundle, gated on Tauri-only); once configured, calls `WebviewWindow::navigate()` to load that URL directly — the exact same `ui/dist` build that `finsight-server` also serves, at which point the page is running against a **remote origin inside the Tauri webview**. Tauri's IPC bridge object stays present on any origin, but command *execution* is origin-scoped by Tauri's own ACL (remote origins get zero grants by default, verified against current docs) — so the frontend's own `isTauriRuntime()` check is updated to also verify it's still on Tauri's *internal* origin (`tauri://localhost` / `http://tauri.localhost` / `https://tauri.localhost`, platform-dependent), not just that the bridge object exists. Once that's true, the app's existing `installHttpBackend()` boot logic kicks in exactly as it does for any browser — no new client-side transport code needed. A native tray icon (show/hide, "Change Server…", quit) is the one desktop affordance kept.

**Tech Stack:** Tauri 2 core (`tauri::tray::TrayIconBuilder`, `WebviewWindow::navigate`), existing `finsight_core::keychain` module, existing React/vite `ui/` bundle (no new frontend framework), axum (finsight-server export routes reuse the existing RPC dispatcher — no new REST surface).

**Spec:** `docs/superpowers/specs/2026-07-15-server-architecture-design.md` (Phase 4: "thin Tauri shell — webview → configured server URL, token in OS keychain, tray icon, native notifications later, no Rust command surface, no local DB"). User confirmed (this session): delete the shipped app's local command surface; `finsight-app` survives only as the codegen source for `bindings.ts`.

**Verified against current Tauri 2 docs during planning** (not assumed): IPC bridge injection is not origin-restricted, but command ACL is — remote origins get zero command grants unless explicitly configured via `dangerousRemoteUrlIpcAccess` (which this plan deliberately never sets, so even a frontend-detection bug can't leak a real command call to a remote page — defense in depth). `WebviewWindow::navigate(url: Url) -> Result<()>` is the exact Rust API for redirecting an existing window. Tauri's own origin is `tauri://localhost` (macOS/Linux) or `http://tauri.localhost` (Windows default) / `https://tauri.localhost` (Windows with `useHttpsScheme: true`, not set in this repo's `tauri.conf.json`).

---

## Ground rules (read first)

- **Baseline (verified green at Phase 3 close):** Rust workspace **591 passed / 0 failed**, frontend **511 tests / 95 files** + `tsc --noEmit` clean, `bindings.ts` byte-identical, `crates/finsight-server/tests/parity.rs` untouched, `finsight-server` confirmed tauri-free (built + ran as a Linux Docker container, `docker inspect` reported `healthy`). Branch: `pwa-desktop-architecture-72a060`.
- **Cargo:** run via **PowerShell**, not Git Bash (Strawberry Perl vs MSYS perl). Cargo tests are **single foreground blocking calls** (`timeout: 600000`); one cargo invocation at a time; `LNK1102` → retry `CARGO_BUILD_JOBS=2`; `LNK1318`/`os error 112` → disk full, BLOCKED.
- **Bindings invariant continues, with a twist:** Tasks 1-5 (the export fix) DO change command signatures (dropping `AppHandle`/dialog params) — bindings.ts **will** change for those 5 commands (expected, not a violation). After Tasks 1-5: regenerate and commit the new bindings.ts, and update `crates/finsight-server/tests/parity.rs`'s expectations only insofar as the `UNSUPPORTED` list shrinks (the parity test itself is generic — it reads `SUPPORTED`/`UNSUPPORTED` consts and compares against bindings.ts automatically; you edit the consts, not the test logic). Tasks 6+ (the shell itself) touch **no** commands — after those, bindings.ts must be byte-identical again (`git diff --exit-code ui/src/api/bindings.ts` → 0).
- **finsight-server stays tauri-free:** `cargo tree -p finsight-server -i tauri` must remain empty throughout — this phase never adds a tauri dependency to finsight-server.
- **`finsight-app` is no longer "the app" — say so in code.** Once Task 8 lands, add a crate-level doc comment to `crates/finsight-app/src/lib.rs` stating plainly that this crate is consumed ONLY by `export_bindings` for TypeScript codegen and is not part of any shipped binary — so a future reader doesn't wonder why 27 files of `#[tauri::command]` wrappers exist with nothing calling `configure_app()`.
- **Origin-aware Tauri detection is the load-bearing correctness fix (Task 6) — do it BEFORE the shell rewrite (Task 8), and prove it with a real navigation test, not just a unit-mocked one** (Task 11's E2E must include an actual `navigate()` to a real running `finsight-server` instance and confirm the HTTP/SSE shim installs correctly on the far side, not just localStorage-level assertions).
- Commit per task, normal commits on top of HEAD.

## File structure (what changes)

```
crates/finsight-api/src/
  csv.rs                          NEW  — shared csv_escape (dedupes 3 private copies)
  commands/
    accounts.rs                   MOD  — export_account_csv: real impl, no AppHandle
    transactions.rs               MOD  — export_transactions_csv, export_search_transactions_csv
                                          (+ moves SearchTxnQueryInput here); real impl
    settings.rs                   MOD  — export_all_data_json, export_all_data_csv; real impl
crates/finsight-app/src/
  lib.rs                          MOD  — crate-level "codegen-only" doc comment; drop now-unused
                                          tauri_plugin_dialog registration/import
  commands/{accounts,transactions,settings}.rs
                                   MOD  — thin delegation for the 5 export wrappers (matches
                                          every other command's pattern now)
crates/finsight-server/src/
  dispatch.rs                     MOD  — UNSUPPORTED shrinks to []; 5 new SUPPORTED arms
ui/src/
  utils/runtime.ts                MOD  — isTauriRuntime() becomes origin-aware
  api/httpBackend.ts               MOD  — install guard uses the same origin-aware check
  lib/downloadBlob.ts             NEW  — tiny Blob→`<a download>` trigger helper
  api/hooks/settings.ts           MOD  — useExportJson/useExportCsv: drop isTauriRuntime() gate,
                                          download via Blob instead of toasting a path
  screens/AccountTransactions.tsx MOD  — handleExport downloads via Blob
  components/copilot/cards/TransactionTableCard.tsx
                                   MOD  — handleExport downloads via Blob
  screens/desktop/ConnectScreen.tsx
                                   NEW  — "Connect to your server" first-run screen (Tauri-only)
  components/DesktopConnectGate.tsx
                                   NEW  — boot-time gate: shows ConnectScreen until a server URL
                                          is configured and reachable, then navigates away
  main.tsx                        MOD  — wires DesktopConnectGate ahead of the normal app tree,
                                          Tauri-only
src-tauri/
  Cargo.toml                      MOD  — drop finsight-app dependency from the `finsight` bin's
                                          runtime path (keep it for export_bindings only — see
                                          Task 8's Cargo.toml split note)
  tauri.conf.json                 MOD  — relax CSP's connect-src/img-src for arbitrary
                                          user-configured https/http origins; keep everything else
  src/main.rs                     MOD  — full rewrite: no configure_app(), keychain-backed
                                          server-url get/set commands, navigate-on-connect, tray
  src/config.rs                   NEW  — get_server_url/set_server_url/clear_server_url Tauri
                                          commands (keychain-backed, NOT part of bindings.ts)
CLAUDE.md                         MOD  — describes the new split (finsight-app=codegen-only,
                                          src-tauri/src/main.rs=shipped thin shell)
docs/self-hosting.md              MOD  — one paragraph: "Desktop app" section replacing the old
                                          implicit assumption that Tauri = full local app
```

---

## Task Group A — Close the export gap (prerequisite; benefits browser/PWA too)

### Task 1: `csv.rs` shared helper + real `export_account_csv`

**Files:** Create `crates/finsight-api/src/csv.rs`; Modify `crates/finsight-api/src/lib.rs` (`pub mod csv;`), `crates/finsight-api/src/commands/accounts.rs`, `crates/finsight-app/src/commands/accounts.rs`.

- [ ] **Step 1: Failing test** (`crates/finsight-api/src/csv.rs`):
```rust
//! Shared CSV-field escaping for the export commands (accounts, transactions,
//! settings) — was triplicated as a private fn in each finsight-app command
//! file before Phase 4 moved the bodies here.
pub fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn escapes_commas_quotes_and_newlines() {
        assert_eq!(csv_escape("plain"), "plain");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
        assert_eq!(csv_escape("a\nb"), "\"a\nb\"");
    }
}
```
Add `pub mod csv;` to `crates/finsight-api/src/lib.rs`.

- [ ] **Step 2: Run** — PowerShell `cargo test -p finsight-api csv::` → PASS immediately (trivial fn; this step exists to confirm the module wires up correctly before the bigger conversion).

- [ ] **Step 3: Convert `export_account_csv`** — in `crates/finsight-api/src/commands/accounts.rs`, add (the account-name-for-filename logic and the dialog are dropped entirely; the caller decides the filename client-side):
```rust
/// Returns the CSV content for one account's transactions (caller downloads
/// it client-side — no server-side file I/O). Real implementation as of
/// Phase 4; previously 501'd behind a native-dialog-only Tauri command.
pub async fn export_account_csv(state: &ApiState, account_id: String) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let mut stmt = conn.prepare(
            "SELECT t.posted_at, t.merchant_raw, COALESCE(c.label,''), t.amount_cents, COALESCE(t.notes,'')
             FROM transactions t
             LEFT JOIN categories c ON c.id = t.category_id
             WHERE t.account_id = ?1
             ORDER BY t.posted_at DESC",
        )?;
        let rows = stmt.query_map(rusqlite::params![account_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
            ))
        })?;
        let mut out = String::from("date,merchant,category,amount_dollars,notes\n");
        for row in rows {
            let (posted_at, merchant, category, amount_cents, notes) = row?;
            let date = &posted_at[..10.min(posted_at.len())];
            let merchant = crate::csv::csv_escape(&merchant);
            let category = crate::csv::csv_escape(&category);
            let amount = format!("{:.2}", amount_cents as f64 / 100.0);
            let notes = crate::csv::csv_escape(&notes);
            out.push_str(&format!("{date},{merchant},{category},{amount},{notes}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
```
(This is the SAME SQL/formatting as the old body — verify by diffing against `git show HEAD:crates/finsight-app/src/commands/accounts.rs` before you edit it, so nothing drifts. The filename-safety/account-name lookup is dropped: the client already knows the account name from its own state and can name the downloaded file itself.)

- [ ] **Step 4: Thin the wrapper** — in `crates/finsight-app/src/commands/accounts.rs`, replace the whole `export_account_csv` body (including its private `csv_escape` — delete it, nothing else in this file uses it) with:
```rust
/// Returns CSV content for one account's transactions; the caller downloads
/// it client-side (Blob + `<a download>`). No native file dialog since Phase 4
/// — the desktop shell has no local command surface to host one.
#[tauri::command]
#[specta::specta]
pub async fn export_account_csv(
    state: tauri::State<'_, AppState>,
    account_id: String,
) -> AppResult<String> {
    finsight_api::commands::accounts::export_account_csv(&state.api, account_id).await
}
```
Remove the now-unused `use tauri_plugin_dialog::DialogExt;` import if `accounts.rs` has no other dialog usage (check — it shouldn't, this was the only exporter in this file).

- [ ] **Step 5: Gates** — `cargo test -p finsight-api` (standalone) PASS; `cargo test -p finsight-app` PASS.
- [ ] **Step 6: Commit** — `git add -A && git commit -m "refactor(server): export_account_csv returns content directly (no native dialog)"`

---

### Task 2: Real `export_transactions_csv` + `export_search_transactions_csv`

**Files:** Modify `crates/finsight-api/src/commands/transactions.rs` (add both fns + move `SearchTxnQueryInput` here), `crates/finsight-app/src/commands/transactions.rs` (thin both wrappers, re-export the moved type).

- [ ] **Step 1: Move `SearchTxnQueryInput`** into `crates/finsight-api/src/commands/transactions.rs` (it currently lives ONLY in finsight-app — confirm with `grep -rn "pub struct SearchTxnQueryInput" crates/` before editing, since if a previous task already relocated it this step is a no-op):
```rust
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchTxnQueryInput {
    pub merchant: Option<String>,
    pub account: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub min_amount_cents: Option<i64>,
    pub direction: Option<String>,
}
```

- [ ] **Step 2: Add both export fns** to `crates/finsight-api/src/commands/transactions.rs`:
```rust
/// Real implementation as of Phase 4 (previously 501'd — dialog-only).
pub async fn export_transactions_csv(state: &ApiState, filter: TxnFilterInput) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let txns = transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: filter.account_id,
                limit: i64::MAX,
                offset: 0,
                search: filter.search,
                filter_preset: filter.filter_preset,
                start_date: filter.start_date,
                end_date: filter.end_date,
            },
        )?;
        let mut out = String::from("date,merchant,category,amount_dollars,notes\n");
        for t in txns {
            let date = t.posted_at.format("%Y-%m-%d").to_string();
            let merchant = crate::csv::csv_escape(&t.merchant_raw);
            let category = crate::csv::csv_escape(t.category_label.as_deref().unwrap_or(""));
            let amount = format!("{:.2}", t.amount_cents as f64 / 100.0);
            let notes = crate::csv::csv_escape(t.notes.as_deref().unwrap_or(""));
            out.push_str(&format!("{date},{merchant},{category},{amount},{notes}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}

/// Real implementation as of Phase 4 (previously 501'd — dialog-only).
pub async fn export_search_transactions_csv(state: &ApiState, query: SearchTxnQueryInput) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        let rows = finsight_core::repos::transactions::search(
            conn,
            &finsight_core::repos::transactions::SearchTxnQuery {
                merchant: query.merchant,
                account: query.account,
                start_date: query.start_date,
                end_date: query.end_date,
                min_amount_cents: query.min_amount_cents,
                direction: query.direction,
            },
            i64::MAX,
        )?;
        let mut out = String::from("date,merchant,category,amount_dollars,account\n");
        for r in rows {
            let date = &r.date[..10.min(r.date.len())];
            let merchant = crate::csv::csv_escape(&r.merchant);
            let category = crate::csv::csv_escape(&r.category);
            let amount = format!("{:.2}", r.amount_cents as f64 / 100.0);
            let account = crate::csv::csv_escape(&r.account);
            out.push_str(&format!("{date},{merchant},{category},{amount},{account}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
```
(Verify these against the current `crates/finsight-app/src/commands/transactions.rs` bodies before committing — same SQL/formatting, dialog+`std::fs::write` dropped, `AppHandle` param dropped.)

- [ ] **Step 3: Thin both wrappers** in `crates/finsight-app/src/commands/transactions.rs`:
```rust
#[tauri::command]
#[specta::specta]
pub async fn export_transactions_csv(
    state: tauri::State<'_, AppState>,
    filter: TxnFilterInput,
) -> AppResult<String> {
    finsight_api::commands::transactions::export_transactions_csv(&state.api, filter).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_search_transactions_csv(
    state: tauri::State<'_, AppState>,
    query: finsight_api::commands::transactions::SearchTxnQueryInput,
) -> AppResult<String> {
    finsight_api::commands::transactions::export_search_transactions_csv(&state.api, query).await
}
```
Add `pub use finsight_api::commands::transactions::SearchTxnQueryInput;` to this file's re-export block (find the existing `pub use finsight_api::commands::transactions::{...}` line and add it there — do not create a second `pub use` block). Delete the now-orphaned private `SearchTxnQueryInput` definition and (if nothing else in the file needs it) the `use tauri_plugin_dialog::DialogExt;` import — check other commands in this file first, `transactions.rs` may still need dialog for something else (it shouldn't, but verify).

- [ ] **Step 4: Gates** — `cargo test -p finsight-api` (standalone) PASS; `cargo test -p finsight-app` PASS.
- [ ] **Step 5: Commit** — `git add -A && git commit -m "refactor(server): export_transactions_csv + export_search_transactions_csv return content directly"`

---

### Task 3: Real `export_all_data_json` + `export_all_data_csv`

**Files:** Modify `crates/finsight-api/src/commands/settings.rs`, `crates/finsight-app/src/commands/settings.rs`.

- [ ] **Step 1: Add both fns** to `crates/finsight-api/src/commands/settings.rs`:
```rust
/// Real implementation as of Phase 4 (previously 501'd — dialog-only).
pub async fn export_all_data_json(state: &ApiState) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        use chrono::Utc;
        use finsight_core::repos::{accounts, goals, rules, transactions};
        let accs = accounts::list_summaries(conn)?;
        let txns = transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: None, limit: i64::MAX, offset: 0, search: None,
                filter_preset: None, start_date: None, end_date: None,
            },
        )?;
        let gs: Vec<serde_json::Value> = goals::list(conn)?
            .into_iter()
            .map(|g| serde_json::json!({
                "id": g.id, "name": g.name, "goalType": g.goal_type,
                "targetCents": g.target_cents, "currentCents": g.current_cents,
                "monthlyCents": g.monthly_cents, "targetDate": g.target_date,
                "color": g.color, "notes": g.notes, "sortOrder": g.sort_order,
                "createdAt": g.created_at,
            }))
            .collect();
        let rs = rules::list_active(conn)?;
        let out = serde_json::json!({
            "exportedAt": Utc::now().to_rfc3339(),
            "accounts": accs, "transactions": txns, "goals": gs, "rules": rs,
        });
        serde_json::to_string_pretty(&out)
            .map_err(|e| finsight_core::CoreError::InvalidState(e.to_string()))
    })
    .await
    .map_err(AppError::from)
}

/// Real implementation as of Phase 4 (previously 501'd — dialog-only).
pub async fn export_all_data_csv(state: &ApiState) -> AppResult<String> {
    let db = (*state.db).clone();
    run(&db, move |conn| {
        use finsight_core::repos::transactions;
        let txns = transactions::list(
            conn,
            transactions::TxnFilter {
                account_id: None, limit: i64::MAX, offset: 0, search: None,
                filter_preset: None, start_date: None, end_date: None,
            },
        )?;
        let mut out = String::from("date,merchant,category,amount_dollars,notes\n");
        for t in txns {
            let date = t.posted_at.format("%Y-%m-%d").to_string();
            let merchant = crate::csv::csv_escape(&t.merchant_raw);
            let category = crate::csv::csv_escape(t.category_label.as_deref().unwrap_or(""));
            let amount = format!("{:.2}", t.amount_cents as f64 / 100.0);
            let notes = crate::csv::csv_escape(t.notes.as_deref().unwrap_or(""));
            out.push_str(&format!("{date},{merchant},{category},{amount},{notes}\n"));
        }
        Ok(out)
    })
    .await
    .map_err(AppError::from)
}
```
(Verify against the current bodies before committing — same query shape, dialog+`std::fs::write` dropped.)

- [ ] **Step 2: Thin both wrappers** in `crates/finsight-app/src/commands/settings.rs`:
```rust
#[tauri::command]
#[specta::specta]
pub async fn export_all_data_json(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::settings::export_all_data_json(&state.api).await
}

#[tauri::command]
#[specta::specta]
pub async fn export_all_data_csv(state: tauri::State<'_, AppState>) -> AppResult<String> {
    finsight_api::commands::settings::export_all_data_csv(&state.api).await
}
```
Delete the file's private `csv_escape` fn (only these two commands used it) and the `use tauri_plugin_dialog::DialogExt;` import (check `delete_all_data`/`get_currency`/etc in this file don't also need it — they shouldn't).

**Note the return-type change:** these two commands previously returned `AppResult<()>` (the dialog path was fire-and-forget); they now return `AppResult<String>` (the actual content). This is a real, deliberate signature change — `bindings.ts` will reflect it, and Task 5 updates the two callers in `ui/src/api/hooks/settings.ts` accordingly.

- [ ] **Step 3: Gates** — `cargo test -p finsight-api` PASS; `cargo test -p finsight-app` PASS.
- [ ] **Step 4: Commit** — `git add -A && git commit -m "refactor(server): export_all_data_json/csv return content directly"`

---

### Task 4: Dispatcher wiring — remove UNSUPPORTED, add 5 SUPPORTED arms

**Files:** Modify `crates/finsight-server/src/dispatch.rs`, `crates/finsight-app/Cargo.toml` (drop `tauri-plugin-dialog` if Task 1-3 removed its last usage — verify with `grep -rln tauri_plugin_dialog crates/finsight-app/src/` first).

- [ ] **Step 1: Update the consts** — `UNSUPPORTED` becomes empty (or is removed entirely if nothing else ever populates it — keep the const, empty, so the parity test's `.chain(UNSUPPORTED)` still compiles and the 501-path code stays in place for future desktop-only commands if any ever appear):
```rust
/// No desktop-only commands remain as of Phase 4 — the 5 former file-dialog
/// exports now return content directly (see Task 1-3 of the Phase 4 plan).
/// Kept as an empty list (not deleted) so the 501/UNSUPPORTED code path stays
/// wired for any future genuinely-desktop-only command.
pub const UNSUPPORTED: &[&str] = &[];
```
Add the 5 commands to `SUPPORTED` (same list, same position discipline as the rest) and add 5 match arms in `dispatch()`:
```rust
"export_account_csv" => ok(c::accounts::export_account_csv(api, arg(&p, "accountId")?).await?),
"export_transactions_csv" => ok(c::transactions::export_transactions_csv(api, arg(&p, "filter")?).await?),
"export_search_transactions_csv" => ok(c::transactions::export_search_transactions_csv(api, arg(&p, "query")?).await?),
"export_all_data_json" => ok(c::settings::export_all_data_json(api).await?),
"export_all_data_csv" => ok(c::settings::export_all_data_csv(api).await?),
```
(Confirm the exact camelCase arg keys against the regenerated `bindings.ts` in Step 3 below — `TxnFilterInput`'s param name is `filter`, `SearchTxnQueryInput`'s is `query`, per the existing wrapper signatures; `account_id` → `"accountId"` per the established convention.)

- [ ] **Step 2: Drop unused dialog plugin (if applicable)** — if `grep -rln tauri_plugin_dialog crates/finsight-app/src/` now returns nothing, remove the `tauri-plugin-dialog` dependency from `crates/finsight-app/Cargo.toml` and its registration (`.plugin(tauri_plugin_dialog::init())`) from `configure_app()` in `crates/finsight-app/src/lib.rs`. (This is safe cleanup, not required for correctness — `finsight-app` never ships anymore after Task 8, but keeping it lean now avoids confusion. Skip this step if the grep still finds a use.)

- [ ] **Step 3: Regenerate bindings + run BOTH parity tests**:
```
cargo run -p finsight-tauri --bin export_bindings
git diff ui/src/api/bindings.ts   # expect real changes: 5 commands lose AppHandle-derived
                                   # differences (none visible in TS — AppHandle isn't exported),
                                   # export_all_data_json/csv change return type Result<null,...>
                                   # → Result<string,...>. Read the diff, confirm it matches.
cargo test -p finsight-server     # both parity tests (routing + arg-key) must pass — they read
                                   # SUPPORTED/UNSUPPORTED + bindings.ts automatically; if your
                                   # arg keys in Step 1 are wrong, the arg-key test goes red with
                                   # the exact mismatch.
```
- [ ] **Step 4: Full gates** — `cargo test --workspace` (PowerShell, jobs=2 if OOM) 0 failures; `cargo tree -p finsight-server -i tauri` empty.
- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat(server): wire the 5 export commands into the RPC dispatcher (UNSUPPORTED is now empty)"`

---

### Task 5: UI — download via Blob instead of native-dialog toast

**Files:** Create `ui/src/lib/downloadBlob.ts` (+ test); Modify `ui/src/api/hooks/settings.ts`, `ui/src/screens/AccountTransactions.tsx`, `ui/src/components/copilot/cards/TransactionTableCard.tsx`.

- [ ] **Step 1: Failing test** (`ui/src/lib/downloadBlob.test.ts`):
```typescript
import { describe, it, expect, vi, afterEach } from "vitest";
import { downloadBlob } from "./downloadBlob";

afterEach(() => vi.restoreAllMocks());

describe("downloadBlob", () => {
  it("creates an object URL, triggers a synthetic download click, and revokes the URL", () => {
    const createUrl = vi.fn(() => "blob:mock-url");
    const revokeUrl = vi.fn();
    vi.stubGlobal("URL", { createObjectURL: createUrl, revokeObjectURL: revokeUrl });
    const clickSpy = vi.fn();
    const origCreateElement = document.createElement.bind(document);
    vi.spyOn(document, "createElement").mockImplementation((tag: string) => {
      const el = origCreateElement(tag);
      if (tag === "a") el.click = clickSpy;
      return el;
    });

    downloadBlob("date,amount\n2026-01-01,10.00\n", "text/csv", "export.csv");

    expect(createUrl).toHaveBeenCalledTimes(1);
    expect(clickSpy).toHaveBeenCalledTimes(1);
    expect(revokeUrl).toHaveBeenCalledWith("blob:mock-url");
  });
});
```
- [ ] **Step 2: Run** — `cd ui && npx vitest run src/lib/downloadBlob.test.ts` → FAIL (module missing).
- [ ] **Step 3: Implement** `ui/src/lib/downloadBlob.ts`:
```typescript
/** Triggers a browser/webview file download from in-memory string content —
 *  the replacement for the old native-save-dialog Tauri commands (Phase 4).
 *  Works identically in a plain browser tab, the installed PWA, and the thin
 *  desktop shell (all three now load the SAME ui/dist bundle; none of them
 *  has a native file-save dialog to call). */
export function downloadBlob(content: string, mimeType: string, filename: string): void {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}
```
- [ ] **Step 4: Run** — PASS.
**Note on `export_account_csv`:** confirmed (via `grep -rn "exportAccountCsv" ui/src`) that this command has NO current UI caller — it's an orphaned command today (exists in `bindings.ts`, never wired into a screen). Task 1 still converts it for consistency (every export command gets the same treatment) and because it'll be needed the moment someone adds a per-account export button, but this plan does NOT add that UI entry point — out of scope, a product decision independent of the desktop-architecture goal.

- [ ] **Step 5: Update the 3 call sites with a caller.**
`ui/src/api/hooks/settings.ts` — `useExportJson`/`useExportCsv` drop the `isTauriRuntime()` gate (these now work everywhere) and download the returned content:
```typescript
export function useExportJson() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.exportAllDataJson();
      if (result.status === "error") throw new Error(result.error.message);
      downloadBlob(result.data, "application/json", "finsight-export.json");
    },
  });
}

export function useExportCsv() {
  return useMutation({
    mutationFn: async () => {
      const result = await commands.exportAllDataCsv();
      if (result.status === "error") throw new Error(result.error.message);
      downloadBlob(result.data, "text/csv", "finsight-transactions.csv");
    },
  });
}
```
Add `import { downloadBlob } from "../../lib/downloadBlob";` at the top; remove the now-unused `isTauriRuntime` import from this file if nothing else in it uses it (check first).

`ui/src/screens/AccountTransactions.tsx` — `handleExport`:
```typescript
const handleExport = async () => {
  try {
    const result = await commands.exportTransactionsCsv(filterValue);
    if (result.status === "ok" && result.data) {
      downloadBlob(result.data, "text/csv", "transactions.csv");
      toast.success("Exported");
    }
  } catch (exportError) {
    toast.error("Export failed", { description: userErrorMessage(exportError, "Try again.") });
  }
};
```
Add the `downloadBlob` import.

`ui/src/components/copilot/cards/TransactionTableCard.tsx` — `handleExport`:
```typescript
async function handleExport() {
  if (!query) return;
  setExporting(true);
  try {
    const result = await commands.exportSearchTransactionsCsv({
      merchant: query.merchant, account: query.account, startDate: query.startDate,
      endDate: query.endDate, minAmountCents: query.minAmountCents, direction: query.direction,
    });
    if (result.status === "ok") {
      if (result.data) {
        downloadBlob(result.data, "text/csv", "transactions.csv");
        toast.success("Exported CSV");
      }
    } else {
      toast.error("Export failed", { description: result.error.message });
    }
  } catch (e) {
    toast.error("Export failed", { description: String(e) });
  } finally {
    setExporting(false);
  }
}
```
Add the `downloadBlob` import.

- [ ] **Step 6: Update/add tests** — the 3 call sites likely have existing tests mocking `commands.export*` to resolve with a path string; update those mocks to resolve with representative CSV/JSON content instead, and assert `downloadBlob` (mock the module) was called with that content — don't just delete coverage. Check `ui/src/screens/AccountTransactions.test.tsx` and any `TransactionTableCard.test.tsx` for existing export-related tests first.
- [ ] **Step 7: Gates** — `cd ui && npx vitest run` (511 baseline + new/changed, 0 failures); `npx tsc --noEmit` clean; `git diff --exit-code ui/src/api/bindings.ts` → 0 (Task 4 already regenerated it; this task must not change it further).
- [ ] **Step 8: Commit** — `git add ui/ && git commit -m "feat(ui): download exports via Blob (works in browser, PWA, and the thin desktop shell)"`

---

## Task Group B — Origin-aware Tauri detection (prerequisite for the shell)

### Task 6: `isTauriRuntime()` becomes origin-aware; `httpBackend.ts` uses the same check

**Files:** Modify `ui/src/utils/runtime.ts` (+ its tests, create if none exist), `ui/src/api/httpBackend.ts` (+ tests).

- [ ] **Step 1: Failing tests** — add to a new/existing `ui/src/utils/runtime.test.ts`:
```typescript
import { describe, it, expect, afterEach, vi } from "vitest";
import { isTauriRuntime } from "./runtime";

const realLocation = window.location;
afterEach(() => {
  vi.unstubAllGlobals();
  Object.defineProperty(window, "location", { value: realLocation, configurable: true });
  delete (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__;
});

function setLocation(origin: string) {
  Object.defineProperty(window, "location", { value: { origin }, configurable: true });
}

describe("isTauriRuntime — origin awareness (Phase 4)", () => {
  it("true when the bridge is present AND on Tauri's own internal origin (mac/linux)", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("tauri://localhost");
    expect(isTauriRuntime()).toBe(true);
  });
  it("true on Tauri's Windows-default internal origin", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("http://tauri.localhost");
    expect(isTauriRuntime()).toBe(true);
  });
  it("true on Tauri's Windows https-scheme internal origin", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("https://tauri.localhost");
    expect(isTauriRuntime()).toBe(true);
  });
  it("FALSE when the bridge is present but the origin is a remote self-hosted server — " +
     "this is the exact Phase 4 shell-after-navigate scenario", () => {
    (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
    setLocation("https://myhost.ts.net");
    expect(isTauriRuntime()).toBe(false);
  });
  it("false when the bridge is absent regardless of origin", () => {
    setLocation("tauri://localhost");
    expect(isTauriRuntime()).toBe(false);
  });
});
```
- [ ] **Step 2: Run** — `cd ui && npx vitest run src/utils/runtime.test.ts` → the "remote origin" and Windows-origin cases FAIL against the current implementation (it only checks bridge presence).
- [ ] **Step 3: Implement** — update `ui/src/utils/runtime.ts`:
```typescript
type TauriWindow = Window & {
  __TAURI__?: unknown;
  __TAURI_INTERNALS__?: unknown;
};

// Tauri's IPC bridge object stays injected on ANY origin the webview navigates
// to, but Tauri's own command ACL is origin-scoped — a remote origin (e.g. the
// user's self-hosted FinSight server, once the Phase 4 desktop shell navigates
// there) gets zero command grants by default. So bridge presence alone is not
// enough to mean "use local Tauri IPC"; the page must also still be on Tauri's
// OWN internal origin. Verified against current Tauri 2 docs: macOS/Linux use
// `tauri://localhost`; Windows defaults to `http://tauri.localhost` and uses
// `https://tauri.localhost` only when `useHttpsScheme: true` is set (not set
// in this repo's tauri.conf.json, but included for robustness).
const TAURI_INTERNAL_ORIGINS = new Set([
  "tauri://localhost",
  "http://tauri.localhost",
  "https://tauri.localhost",
]);

export function isTauriRuntime() {
  const meta = import.meta as { env?: { MODE?: string; VITEST?: string } };
  if (meta.env?.MODE === "test" || meta.env?.VITEST) return true;
  if (typeof window === "undefined") return false;
  if (typeof navigator !== "undefined" && navigator.userAgent.includes("jsdom")) return true;
  const w = window as TauriWindow;
  if (!(w.__TAURI__ || w.__TAURI_INTERNALS__)) return false;
  return TAURI_INTERNAL_ORIGINS.has(window.location.origin);
}
```
- [ ] **Step 4: Run** — all 5 new tests PASS; run the FULL suite (`cd ui && npx vitest run`) to confirm no existing test broke — several screens/hooks call `isTauriRuntime()` and their tests likely run under vitest (which short-circuits `true` via the `meta.env?.MODE === "test"` branch, UNCHANGED), so this should be a no-op for all of them; if anything breaks, it's a test that stubbed `window.__TAURI_INTERNALS__` directly while running outside the vitest short-circuit — read it and fix the stub to also set a matching origin, don't weaken the new check.

- [ ] **Step 5: Apply the SAME check in `httpBackend.ts`** — its `installHttpBackend()` guard (`if (w.__TAURI_INTERNALS__) return;`) has the identical gap: after the shell navigates to a remote server, this guard would incorrectly bail out (bridge present) and never install the HTTP/SSE shim. Replace the raw check with the shared helper:
```typescript
// httpBackend.ts, top of file
import { isTauriRuntime } from "../utils/runtime";

export function installHttpBackend(): void {
  const w = window as unknown as AnyRec;
  if (isTauriRuntime()) return; // never shadow a real Tauri runtime (origin-aware, Phase 4)
  // ...unchanged below
```
- [ ] **Step 6: Extend `httpBackend.test.ts`** with one new case: bridge present + remote origin → `installHttpBackend()` proceeds (does NOT early-return) — assert `window.__TAURI_INTERNALS__.invoke` gets overwritten to the fetch-based one (i.e., calling it hits `fetch`, not the pre-existing stub). Mirror the existing test's setup style.
- [ ] **Step 7: Gates** — `cd ui && npx vitest run` (green) + `npx tsc --noEmit` clean.
- [ ] **Step 8: Commit** — `git add ui/ && git commit -m "fix(ui): isTauriRuntime is origin-aware — a Tauri webview navigated to a remote server is server-mode, not desktop-IPC-mode"`

---

## Task Group C — The thin shell

### Task 7: Server-URL config screen (React) + keychain-backed get/set commands (Rust)

**Files:** Create `src-tauri/src/config.rs`, `ui/src/screens/desktop/ConnectScreen.tsx` (+ test), `ui/src/components/DesktopConnectGate.tsx` (+ test); Modify `src-tauri/Cargo.toml` (no new deps — reuses `finsight-core`'s keychain + `tauri`), `ui/src/main.tsx`.

- [ ] **Step 1: `src-tauri/src/config.rs`** — 3 tiny Tauri commands, deliberately NOT part of `bindings.ts` (they're internal to the shell, never called by the shared `ui/dist` app once it's loaded from a real server — only the local `ConnectScreen` calls them, via raw `@tauri-apps/api` `invoke`, not the generated `commands` object):
```rust
//! Server-URL storage for the thin desktop shell (Phase 4). Deliberately NOT
//! part of the generated bindings.ts — these 3 commands only exist for the
//! shell's own local ConnectScreen, called via raw `invoke()`, not the shared
//! command surface the rest of the app (browser/PWA/post-navigate shell) uses
//! over HTTP. finsight_core::keychain is already generic over (service, user)
//! — reused as-is, no changes to that module.
const SERVICE: &str = "com.finsight.desktop";
const USER: &str = "server_url";

#[tauri::command]
pub fn get_server_url() -> Result<Option<String>, String> {
    finsight_core::keychain::get_key(SERVICE, USER).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_server_url(url: String) -> Result<(), String> {
    finsight_core::keychain::set_key(SERVICE, USER, &url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_server_url() -> Result<(), String> {
    finsight_core::keychain::delete_key(SERVICE, USER).map_err(|e| e.to_string())
}
```
(No `#[specta::specta]` — these never go into `bindings.ts`. Return `Result<_, String>` per plain-Tauri-command convention, not `AppResult` — `finsight-api`'s `AppError` type isn't relevant here, this file has zero `finsight-api`/`finsight-app` dependency.)

- [ ] **Step 2: `ui/src/screens/desktop/ConnectScreen.tsx`** — a minimal, real screen (uses the same `.card`/`.btn`/tokens conventions as the rest of the app; it's shipped in the SAME `ui/dist` bundle, just conditionally rendered):
```tsx
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

/** First-run screen for the Phase 4 thin desktop shell: ask for the
 *  self-hosted FinSight server URL, verify it's reachable, store it in the
 *  OS keychain, then hand off to the caller (DesktopConnectGate) to navigate
 *  the window there. Only ever rendered inside the bundled shell app — never
 *  reachable once the window has navigated to a real server (that's a
 *  different origin serving the same ui/dist build, minus this screen's route
 *  ever being hit, since DesktopConnectGate only mounts pre-navigation). */
export default function ConnectScreen({ onConnected }: { onConnected: (url: string) => void }) {
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [checking, setChecking] = useState(false);

  async function handleConnect(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const normalized = url.trim().replace(/\/+$/, "");
    if (!normalized) { setError("Enter your server's address."); return; }
    setChecking(true);
    try {
      const res = await fetch(`${normalized}/api/health`, { method: "GET" });
      if (!res.ok) throw new Error(`Server responded ${res.status}`);
      const body = await res.json();
      if (body.status !== "ok") throw new Error("Unexpected response from server");
      await invoke("set_server_url", { url: normalized });
      onConnected(normalized);
    } catch (err) {
      setError(
        err instanceof Error
          ? `Couldn't reach that server: ${err.message}`
          : "Couldn't reach that server."
      );
    } finally {
      setChecking(false);
    }
  }

  return (
    <div className="onb-stage" style={{ maxWidth: 440, margin: "80px auto" }}>
      <div className="card">
        <div className="eyebrow">Connect to your server</div>
        <h1 className="h1" style={{ marginTop: 8 }}>Where's your FinSight server?</h1>
        <p className="muted">
          Enter the address of your self-hosted FinSight server — for example a
          Tailscale hostname, a local network address, or a domain name.
        </p>
        <form onSubmit={handleConnect} style={{ marginTop: 20 }}>
          <input
            type="text"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="https://finsight.example.ts.net"
            autoFocus
            style={{ width: "100%" }}
          />
          {error && <p role="alert" style={{ color: "var(--negative)", marginTop: 8 }}>{error}</p>}
          <button className="btn primary" type="submit" disabled={checking} style={{ marginTop: 16 }}>
            {checking ? "Connecting…" : "Connect"}
          </button>
        </form>
      </div>
    </div>
  );
}
```
- [ ] **Step 2b: Test** (`ConnectScreen.test.tsx`) — mock `fetch` and `@tauri-apps/api/core`'s `invoke`: successful health check → calls `set_server_url` then `onConnected`; non-ok health check → shows error, does NOT call `onConnected`; network failure → shows error.

- [ ] **Step 3: `ui/src/components/DesktopConnectGate.tsx`** — the boot-time gate, Tauri-only:
```tsx
import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { isTauriRuntime } from "../utils/runtime";
import ConnectScreen from "../screens/desktop/ConnectScreen";

/** Only relevant to the bundled desktop shell (isTauriRuntime() — which is
 *  now origin-aware, see Phase 4 Task 6 — so this correctly stops rendering
 *  once the window has navigated to a real server, since at that point the
 *  page is no longer on Tauri's internal origin). Elsewhere (browser, PWA,
 *  post-navigate shell) this renders children immediately with zero effect. */
export default function DesktopConnectGate({ children }: { children: React.ReactNode }) {
  const [state, setState] = useState<"checking" | "needsConnect" | "connecting">("checking");

  useEffect(() => {
    if (!isTauriRuntime()) { setState("connecting"); return; }
    let alive = true;
    invoke<string | null>("get_server_url").then((url) => {
      if (!alive) return;
      if (url) {
        setState("connecting");
        getCurrentWebviewWindow().navigate(url).catch(() => setState("needsConnect"));
      } else {
        setState("needsConnect");
      }
    }).catch(() => { if (alive) setState("needsConnect"); });
    return () => { alive = false; };
  }, []);

  if (!isTauriRuntime() || state === "connecting") return <>{children}</>;
  if (state === "checking") return null; // avoid a flash of ConnectScreen while checking
  return <ConnectScreen onConnected={(url) => getCurrentWebviewWindow().navigate(url)} />;
}
```
(`getCurrentWebviewWindow().navigate(url)` is the frontend JS equivalent of the Rust `WebviewWindow::navigate` — confirm the exact `@tauri-apps/api` v2 export name against `ui/node_modules/@tauri-apps/api` during implementation; adapt if the API surface differs slightly from this sketch, but the semantic — navigate the CURRENT window to an arbitrary URL — is what Task 6 assumes to trigger the origin change.)

- [ ] **Step 3b: Test** — `get_server_url` resolves a URL → `navigate()` called with it, `ConnectScreen` never renders; resolves `null` → `ConnectScreen` renders; non-Tauri (`isTauriRuntime()` false, the default in vitest is `true` per its short-circuit — so this specific case needs an explicit override matching how other Phase 2/3 tests stub `window.__FINSIGHT_HTTP__`/mock the module) → renders `children` immediately, `invoke` never called.

- [ ] **Step 4: Wire into `main.tsx`** — wrap the render tree with `<DesktopConnectGate>` as the OUTERMOST element (before `AuthGate`/providers — a desktop shell with no configured server has no server to check auth status against, so this must gate before anything that assumes a reachable backend):
```tsx
root.render(
  <React.StrictMode>
    <DesktopConnectGate>
      {/* existing tree: QueryClientProvider/PersistQueryClientProvider → AuthGate → banners → router */}
    </DesktopConnectGate>
  </React.StrictMode>
);
```
(Read the CURRENT exact `main.tsx` render structure first — Phase 3 Task 4 already nested several providers here; preserve all of it, just add one more outer wrapper.)

- [ ] **Step 5: Gates** — `cd ui && npx vitest run` (green, count grows by new tests) + `npx tsc --noEmit` clean. Do NOT check `bindings.ts` in this task — no command surface changed (config.rs commands are deliberately unbound from specta).
- [ ] **Step 6: Commit** — `git add -A && git commit -m "feat(desktop): server-URL connect screen + keychain storage + navigate-on-connect"`

---

### Task 8: Rewrite `src-tauri/src/main.rs` — the actual thin shell

**Files:** Modify `src-tauri/src/main.rs`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`; Create nothing new (config.rs already exists from Task 7).

This is the task that makes the shipped binary stop depending on `finsight-app`'s command surface at runtime. **`export_bindings` must keep working** — it's a separate `[[bin]]` in the same crate and only needs `finsight_app::build_specta_builder()`, never `configure_app()`, so it is UNAFFECTED by this task as long as `finsight-app` stays a workspace member with its 199 wrappers intact (they are — Tasks 1-6 only changed 5 of them, and only their internals, not their existence).

- [ ] **Step 1: `src-tauri/Cargo.toml`** — the crate keeps `finsight-app` as a dependency (still needed by the `export_bindings` bin target), but the `finsight` bin's `main.rs` will simply not import/call anything from it except nothing at all — no Cargo.toml change is strictly required here; Rust doesn't compile out an unused workspace dependency per-binary within one crate automatically, but this is fine (the compiled size cost is `finsight-app`'s code existing in the same compilation, not necessarily linked into the `finsight` binary's live call graph — verify this is acceptable; if `cargo bloat` or binary size becomes a concern later, splitting `export_bindings` into its own crate is a valid future follow-up, explicitly out of scope here). No Cargo.toml edits needed for this step.

- [ ] **Step 2: Add the tray + notification-icon assets check** — confirm `src-tauri/icons/` already has the icons `tauri.conf.json` references (it does, per the existing bundle config) — Tauri 2's `TrayIconBuilder` can reuse `app.default_window_icon()` for the tray icon with no new asset needed:
```rust
// inside setup(), after building the tray:
.icon(app.default_window_icon().cloned().unwrap())
```

- [ ] **Step 3: Rewrite `src-tauri/src/main.rs`** in full:
```rust
// Thin desktop shell (Phase 4): no local command surface, no local database.
// Reads a server URL from the OS keychain (crates/finsight_app is NOT used
// here — it exists only for the `export_bindings` bin's TypeScript codegen).
// On first launch (no stored URL) the bundled ui/dist app shows its own
// ConnectScreen (gated by DesktopConnectGate, itself gated on isTauriRuntime()
// — see ui/src/utils/runtime.ts); once a URL is set, the window navigates
// there directly and behaves exactly like the browser/PWA from that point on.

mod config;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Focus the existing window instead of opening a second one — no
            // local-DB-lock reason to enforce this anymore, but two windows
            // of the same shell is still bad UX.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            config::get_server_url,
            config::set_server_url,
            config::clear_server_url,
        ])
        .setup(|app| {
            let change_server = MenuItemBuilder::with_id("change-server", "Change Server…").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&change_server, &quit]).build()?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().cloned().unwrap())
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "change-server" => {
                        if let Some(window) = app.get_webview_window("main") {
                            // Navigate back to the shell's own bundled origin
                            // and clear the stored URL — the app's own boot
                            // logic (DesktopConnectGate) then re-shows
                            // ConnectScreen because get_server_url() is None.
                            let _ = finsight_core::keychain::delete_key("com.finsight.desktop", "server_url");
                            let _ = window.emit("finsight-desktop:reset", ());
                            if let Ok(app_url) = window.config().build.dev_url.clone()
                                .map(|u| u.to_string())
                                .ok_or(())
                                .or_else(|_| Ok::<_, ()>("tauri://localhost".to_string()))
                            {
                                let _ = window.navigate(app_url.parse().unwrap());
                            }
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => std::process::exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FinSight desktop shell");
}
```
**Flag for implementation-time verification, don't guess:** the "navigate back to the app's own bundled origin" logic above (for the tray's "Change Server…" item) needs the exact right target URL — in a PRODUCTION build this should be the app's own internal origin (`tauri://localhost` / `http://tauri.localhost` per Task 6's constants, NOT `devUrl`); in a DEV build (`pnpm tauri:dev`) it should be `http://localhost:5173`. Read how `tauri.conf.json`'s `build.frontendDist`/`devUrl` are actually exposed to Rust at runtime (likely via `app.config()`, not literally `window.config().build.dev_url` as sketched above — that API name is a placeholder, confirm the real one against the Tauri 2 `Config` struct docs before finalizing) and write this correctly; do not ship a guess. Also emit the `finsight-desktop:reset` event correctly if `DesktopConnectGate` is going to listen for it (Step 4 below) — or simplify by just reloading the window (`window.eval("location.reload()")` or `window.reload()` if that's the real API name) instead of trying to construct the local origin URL by hand, which sidesteps the whole dev/prod URL question. **Prefer the reload-based approach if the Tauri API supports it** — simpler and correct in both dev and prod without needing to know the app's own origin string at all.

- [ ] **Step 4 (only if the reload approach from Step 3's note isn't sufficient):** wire `DesktopConnectGate` to listen for a `finsight-desktop:reset` Tauri event and reset its own `state` to `"checking"` so it re-queries `get_server_url()` after a reload/navigate-back. If the reload approach works cleanly (a full page reload re-runs `main.tsx`'s boot logic from scratch, which naturally re-checks `get_server_url()`), this step is unnecessary — determine which during implementation and don't build unused plumbing.

- [ ] **Step 5: `tauri.conf.json` CSP** — the current CSP's `connect-src 'self' ipc: http://ipc.localhost` only allows the app's own origin plus Tauri's IPC scheme; once the window navigates to an arbitrary user-configured server, that server's own page (served from `finsight-server`) needs to `fetch()` its OWN origin (which becomes `'self'` again post-navigation, since CSP is evaluated per-document/per-origin) — so no CSP change should actually be needed for THIS reason. The one real gap: the `ConnectScreen`'s pre-navigation health-check `fetch()` call (Task 7 Step 2) runs from the STILL-LOCAL origin (`tauri://localhost` etc.) but targets an ARBITRARY remote origin (`connect-src` currently forbids this — only `'self' ipc: http://ipc.localhost` are allowed). Relax `connect-src` to also allow `https:` and `http:` schemes generally, since the whole point is connecting to a user-chosen origin unknown at build time:
```json
"csp": "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' https://fonts.gstatic.com; connect-src 'self' ipc: http://ipc.localhost https: http:"
```
Document this relaxation with a comment in the plan/commit message: it's a deliberate, necessary loosening (the app must be able to reach whatever server the user configures, which is unknowable at build time), not an oversight — and it only affects the LOCAL bundled origin's outbound fetches (the ConnectScreen's health check); it does not grant the remote server's own page any elevated Tauri capabilities (that's governed by the separate ACL mechanism verified in Task 6's research, untouched by CSP).

- [ ] **Step 6: Gates** — `cargo build -p finsight-tauri --bin finsight` (PowerShell, foreground) compiles clean; `cargo build -p finsight-tauri --bin export_bindings` STILL compiles clean (proves the codegen path survives); `cargo run -p finsight-tauri --bin export_bindings && git diff --exit-code ui/src/api/bindings.ts` → 0 (this task adds zero commands to the shared surface — `config.rs`'s 3 commands are deliberately unspecta'd and invisible to bindings.ts).
- [ ] **Step 7: Commit** — `git add -A && git commit -m "feat(desktop): rewrite the shipped Tauri binary as a thin shell (tray, server-URL config, no local command surface)"`

---

### Task 9: `finsight-app` codegen-only doc comment + CLAUDE.md/docs updates

**Files:** Modify `crates/finsight-app/src/lib.rs`, `CLAUDE.md`, `docs/self-hosting.md`.

- [ ] **Step 1:** Add to the top of `crates/finsight-app/src/lib.rs`:
```rust
//! **Codegen-only as of Phase 4.** This crate's ~199 `#[tauri::command]`
//! wrappers are consumed SOLELY by `src-tauri`'s `export_bindings` binary
//! (via `build_specta_builder()`) to generate `ui/src/api/bindings.ts`.
//! Nothing in this crate is linked into the SHIPPED desktop binary anymore —
//! that's `src-tauri/src/main.rs`, a thin webview shell with no local
//! command surface and no local database (see
//! docs/superpowers/plans/2026-07-17-server-phase4-thin-desktop-shell.md).
//! If you're reading this wondering why 27 files of Tauri commands exist
//! with nothing calling `configure_app()` in the shipped app: this is why.
```
- [ ] **Step 2: `CLAUDE.md`** — update the `**crates/finsight-app**` bullet in the Architecture section to reflect the above (one sentence addition: "As of Phase 4, this crate is codegen-only — never shipped; see `src-tauri/src/main.rs` for the actual desktop binary."); add a `pnpm tauri:dev`-adjacent note under Commands clarifying that `tauri:dev` now launches the thin shell (which will show the ConnectScreen against whatever `cargo run -p finsight-server` instance you point it at — not a local DB).
- [ ] **Step 3: `docs/self-hosting.md`** — add one short "Desktop app" section: the shell is downloaded/built once, then connects to your server URL like any other client; "Change Server…" is in the tray menu; the shell has no offline mode of its own (it's the same web app, so the Phase 3 PWA offline cache does NOT apply inside the native shell unless the shell is also treated as `isServerMode()` — verify this during Task 11 and note the actual behavior here, don't assume).
- [ ] **Step 4: Commit** — `git add -A && git commit -m "docs: finsight-app is codegen-only; document the thin desktop shell"`

---

### Task 10: End-to-end verification (Phase 4 exit criterion)

- [ ] **Step 1: Full green bar** — `cargo test --workspace` (PowerShell; jobs=2 if OOM) 0 failures; `cd ui && npx vitest run && npx tsc --noEmit` green; `cargo run -p finsight-tauri --bin export_bindings; git diff --exit-code ui/src/api/bindings.ts` → 0; `cargo tree -p finsight-server -i tauri` empty.
- [ ] **Step 2: Build both binaries** — `cargo build -p finsight-tauri --bin finsight` and `--bin export_bindings`, both succeed.
- [ ] **Step 3: Launch a real `finsight-server`** against a scratch data dir (as in Phases 1-3) and run the shell against it:
  1. First launch, no stored server URL → ConnectScreen appears.
  2. Enter the running server's URL (e.g. `http://localhost:8674`) → health check succeeds → window navigates → the REAL app loads (setup wizard or login, depending on whether that scratch server already has an admin).
  3. **The critical proof (Task 6's whole point):** confirm the app is NOT stuck trying Tauri IPC after navigation — check that `installHttpBackend()` actually installed (e.g. via devtools: `window.__FINSIGHT_HTTP__ === true` after navigation) and that RPC calls succeed (create an account, confirm it persists) — this is empirical proof the origin-aware fix works against a REAL navigation, not just the unit-mocked version.
  4. Close and relaunch the shell (same OS keychain, same server) → skips ConnectScreen, navigates straight to the stored URL, session cookie (Tauri's webview persists cookies across restarts, same as a real browser profile — confirm this empirically here, don't assume) either still valid or prompts login depending on session TTL.
  5. Tray: left-click shows/focuses the window; "Change Server…" clears the stored URL and returns to ConnectScreen (confirm via Step 3's chosen mechanism — reload or explicit navigate-back).
  6. Exports: trigger a CSV export from the running shell — confirm a real file download happens (Blob → native OS save dialog is the WEBVIEW's own default download handling for `<a download>`, not anything FinSight built — this should just work via the OS's normal download UX).
- [ ] **Step 4: Record results** in Linear (create the Phase 4 project/issues mirroring Phases 1-3's pattern before starting implementation, not after) + update the plan's checkboxes. Then `superpowers:finishing-a-development-branch`.

---

## Explicitly out of scope (Phase 4)

- **Native OS notifications bridged from server-side SSE events** — the spec says "native notifications later"; this plan's shell has zero notification-plugin usage. A future pass: listen to the shell's own SSE connection (once past ConnectScreen, it's just a browser tab technically, so this would need a small bridge — e.g. a lightweight Tauri command the post-navigation page can still reach IF that specific command is granted remote ACL access via `dangerousRemoteUrlIpcAccess` scoped ONLY to that one command, which is a deliberate, narrow exception worth its own design pass, not bundled here).
- **Auto-launch on system startup** — common desktop-app nicety, not required for MVP.
- **Multiple saved servers / quick-switch** — "Change Server…" clears and re-prompts; no server history/list.
- **`export_bindings` architecture cleanup** (splitting it out of `finsight-app`/`src-tauri` into its own minimal crate so `finsight-app`'s 2705 lines aren't compiled into the same crate as a real shipped binary's build graph at all) — noted as a valid future simplification in Task 8, not done here; current approach (codegen-only, unlinked) is correct and suffices.
- **CSV share-target, offline mutation editing, mobile bottom-nav** — unrelated Phase 3 deferred items, still parked (see `docs/superpowers/plans/2026-07-17-server-phase3-pwa-docker.md`'s own out-of-scope section and memory).
