# Adversarial code review — `origin/main...HEAD`

Scope: only the changed lines in this branch's diff (scenarios.rs SQL fix + tests,
Scenarios.tsx, Settings.tsx/app.css container query, index.html, mockBackend.ts,
new auth screens + authScene.tsx + auth.css).

## Summary

No high-confidence **high-severity** (crash / data-loss / broken-flow) bugs were
found. The diff is clean. The items below are one medium and two low findings,
plus explicit confirmations of the areas the review was asked to focus on.

## Findings

### 1. [MED] `prefers-reduced-motion` is honored for CSS animations but not the JS-driven ones
`ui/src/screens/server/authScene.tsx` (whole file) + `ui/src/styles/auth.css:538`

`auth.css` line 538 has a `@media (prefers-reduced-motion: reduce)` block that
disables the CSS card-drift / sheen / mark-pulse animations — so the author
clearly intended to respect the setting. But every JavaScript-driven animation in
`authScene.tsx` runs unconditionally: the mesh + constellation background
(`useShowcaseBg`, continuous `requestAnimationFrame`), the mouse/device-tilt 3D
parallax loop (`Showcase` effect, `authScene.tsx:412`), the three count-ups
(`useCountUp`), and the sparkline / hero line draws. There is no `matchMedia`
check anywhere in the file (verified). 

Failure scenario: a user who has set "reduce motion" at the OS level opens the
setup/login/recovery screen and still gets a full-motion animated particle field,
a card stack that tilts as the cursor moves, and animated counting numbers.
Because the author's own CSS shows the intent to suppress motion, this is an
incomplete implementation rather than a design choice. Fix: gate the rAF loops
(or at least the parallax + background) behind
`window.matchMedia("(prefers-reduced-motion: reduce)").matches`.

### 2. [LOW] Recovery screen has no new-password length guidance or validation; inconsistent with Setup
`ui/src/screens/server/RecoverScreen.tsx` (handleSubmit ~line 34) and
`ui/src/screens/server/SetupScreen.tsx:31`

`SetupScreen` now enforces `MIN_PASSWORD_LEN = 10` client-side. `RecoverScreen`'s
`handleSubmit` only checks that fields are non-empty and that the two passwords
match — no minimum length. The diff also removed the old `hint="At least 10
characters."` that used to sit on the recover new-password input, so the user now
gets neither a hint nor client validation. If the server enforces a minimum they
see a server error; if it doesn't, they can set a weaker password via recovery
than setup would allow. This is largely pre-existing (recover never validated
length), so it's low severity, but the removed hint is a small regression in
guidance. The password strength meter's first threshold is also 8, not 10, so a
password can read "GOOD"/"STRONG" and still be rejected by Setup's 10-char rule.

### 3. [LOW] `index.html` dark-first default causes a brief FOUC for light/compact users
`ui/index.html:8`

`<html data-theme="dark" data-density="cozy">` is now hardcoded. `ThemeProvider`
(`ui/src/components/ThemeProvider.tsx:9-10`) unconditionally re-applies the saved
theme/density on mount, so a user who saved `light` / `compact` sees one frame of
dark-cozy before `<App/>` reconciles. This is intentional (documented in the HTML
comment — it fixes transparent auth inputs that render before ThemeProvider) and
acceptable; noted only for completeness.

## Confirmations (focus areas — verified OK)

**scenarios.rs SQL fix is complete.** Reconstructing the joined multi-line string,
the only broken boundary was `FROM transactions` + `WHERE` → `transactionsWHERE`
(identifier fused to a keyword), now fixed by the trailing space. Every other
line-continuation boundary is valid SQL: `-amount_cents ` keeps its space before
`WHEN`/`ELSE`; `* 12` + `+ (CAST` → `12+ (CAST` (valid arithmetic); `+ 1,` + `1)`
→ comma-separated COALESCE args; and `1)` + `FROM` → `1)FROM` is fine because `)`
is a self-terminating token. No other joined tokens remain.

**The regression tests genuinely catch it.** `build_baseline_runs_on_empty_db`
calls `build_baseline(...).unwrap()`; against the pre-fix query the `conn.query_row(...)?`
returns a SQL error, so `unwrap()` panics and the test fails — a true guard.
`build_baseline_averages_income_and_expense` correctly exercises the sum/span math
(both txns dated today ⇒ span clamps to 1 ⇒ avg == sum). Verified the test module
compiles: `seed_txn`'s `NewTransaction` matches all 14 struct fields;
`seed_account`'s `NewAccount` literal provides all 35 fields with matching names
(no `..Default`); `accounts::insert` returns `CoreResult<Account>` and `Account.id`
is a `String` (so `.unwrap().id` is valid); `transactions::insert` takes
`NewTransaction`.

**authScene.tsx resource cleanup is complete.** Every effect tears down what it
creates: `useCountUp` (clearTimeout + cancelAnimationFrame), `Sparkline` /
`HeroChart` (same pair), `useShowcaseBg` (cancelAnimationFrame + `ro.disconnect()`),
`Showcase` parallax effect (cancelAnimationFrame + removes `mousemove`,
`mouseleave` on the host and `deviceorientation` on window), `Showcase` caption
`setInterval` (clearInterval). All `raf`/`to` handles are `let`-scoped in the
effect and shared with their cleanup closures, so the latest id is cancelled — no
leak across AuthGate screen swaps (Login↔Recover) or StrictMode double-mount.

**Canvas null-safety and array bounds are handled.** Every `getContext("2d")` is
null-checked (`Sparkline`, `HeroChart`, `useShowcaseBg` checks both contexts).
`smoothCurve` indices are all bounded for `n >= 3` (it early-returns a copy for
`n < 3`), and every draw-loop input has ≥ 3 points; divide-by-zero is guarded
(`max - min || 1`, and the hero's `min/max` are padded ±1.5).

**Auth wiring is correct.** `login(username.trim(), password)`,
`setup(username.trim(), password)`, `recoverAccount(username.trim(),
recoveryKey.trim(), newPassword)` — all match the `auth.ts` signatures and read
`.recoveryKey` off the result. No silent no-op or double-submit: submit buttons
are `disabled` while pending, the eye-toggle and "Forgot password?" buttons are
`type="button"`, and the real inputs are keyboard-reachable (`id`+`htmlFor`
label association); the showcase is `aria-hidden`.

**auth.css is fully scoped.** Every selector is prefixed with `.fs-auth`; no
`:root`, bare-element, or global-class rule escapes. The five keyframes
(`markpulse`, `authsheen`, `authspin`, `authcardin`, `authdrift`) are unique — no
collision with app.css (verified by grep).

**Scenarios.tsx key-reset + applied-ids logic is correct.** `PromotePanel` is
keyed by `proposal.scenarioId` and `RevisePanel` by `revising.id`, so switching
scenarios remounts and resets stale `approved`/`appliedIds`/`result` and seeded
revise inputs. The double-apply guard is sound: applied ids are removed from
`approved` and added to `appliedIds`; the checkbox becomes checked+disabled; the
"Apply N to plan" button is `disabled` on `approved.size === 0` and while pending,
so an already-written change can't be applied twice.

**Settings container query is wired correctly.** `.screen-settings` (which carries
`container-type: inline-size`) is the Settings root (`Settings.tsx:648`), so the
`@container (max-width: 760px)` rule has a valid container ancestor.

**mockBackend.ts** is dev-harness-only (not shipped); its `recur` sign change and
new responders are internally consistent and out of scope for user-facing bugs.
