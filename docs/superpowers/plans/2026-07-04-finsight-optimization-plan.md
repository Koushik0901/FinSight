Phase 7 Goal: Performance, parallelization, anticipatory computation, and responsiveness hardening.

Make FinSight faster and immediate without changing correct behavior. Profile first, optimize bottlenecks, and use robust Rust/Tauri, SQL, React, and React Query practices. Write production-quality code. Test concurrency, cancellation, stale results, cache invalidation, and edge cases. Never trade correctness for speed. Use real data from `samples/`.

CORE PRINCIPLE — COMPUTE AS EARLY AS SAFELY POSSIBLE:
Across the app, identify the earliest moment enough information exists to begin non-destructive backend work. Parse, validate, analyze, precompute, prepare, and warm safe caches before the user reaches the final action. The UI should feel instant because prerequisites already ran.

CSV example: when a file is selected, immediately read it; detect format/header/date/amount conventions and likely columns; validate/preview rows; normalize merchants; detect duplicates; and build a prepared import plan. As mappings become known, run dependent parsing/validation. Precompute safe statistics, date ranges, category candidates, and recomputation plans where possible.

Do not wait for final Import to start work needing no confirmation. Ideally, Import commits prepared validated data, then runs only work requiring persisted state. After commit, categorization and affected charts, reports, recurring analysis, insights, and caches must recompute through an explicit dependency pipeline.

Apply this generically to search/filters, account selection, category edits, recategorization previews, reports, charts, Copilot tools, scenarios, approvals, and delete/reset. Trigger work once prerequisites exist.

Rules:

* speculative work stays non-destructive until confirmation; never commit mutations early
* cancel/discard work when inputs change/dialogs close
* use request/version IDs so stale results cannot overwrite newer state
* deduplicate in-flight work and reuse prepared results
* bound concurrency/memory; preserve deterministic outputs
* avoid races, DB contention, stale caches

Tasks:

1. Record before/after baselines for startup, import, transactions, dashboard/reports/charts, categorization, Copilot latency, delete/re-import, memory, and responsiveness.
2. Profile Rust/Tauri commands, SQL, IPC/serialization, blocking work, React renders/refetches, chart/table cost, and roundtrips.
3. Backend: batch writes; use DB transactions correctly; add justified indexes; remove N+1 queries; reduce cloning/serialization/per-row IPC; move CPU work off UI-sensitive paths; parallelize only safe independent work; add bounded orchestration, cancellation, progress events, stale-result rejection, and reusable prepare/commit pipelines; control DB write contention.
4. Frontend: reduce rerenders/refetch storms; tune caching/invalidation; reuse prepared results; debounce/cancel superseded work; virtualize/lazy-load/prefetch where justified; keep states truthful.
5. Copilot: run independent deterministic tool/context work concurrently; avoid excessive raw transaction payloads; context-pack backend summaries; cache safe results with precise invalidation; preserve streaming/cancellation; never reuse deleted/stale data.
6. Test rapid input changes, cancelled dialogs, repeated clicks, concurrent requests, duplicate imports, stale responses, empty/large datasets, DB failures, and Delete All Data during in-flight work. Delete must cancel work and invalidate caches, prepared plans, and derived state.

Acceptance criteria:

* baselines/bottlenecks documented with evidence
* major flows measurably faster or feel instant because safe work starts early
* final actions avoid unnecessary recomputation
* stale/speculative work cannot mutate or overwrite current state
* UI stays responsive and invalidation remains correct
* correctness and approval safety remain intact
* tests/benchmarks pass

Final report: baselines, bottlenecks, anticipatory pipelines, optimizations, timings, concurrency/cancellation, tests, cache correctness, remaining risks.
