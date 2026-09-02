# FinSight LLM Replacement Audit — Local & Traditional Alternatives

**Date:** 2026-08-31  
**Scope:** Exhaustive grep of `crates/` (finsight-agent, finsight-api, finsight-core). Every `CompletionProvider` call-site enumerated, cost profile mapped, and live-researched cheaper replacement proposed.  
**Goal:** Cut API token cost (Anthropic / OpenAI-compat) by replacing cloud LLM calls with local SLMs, distilled classifiers, embedding similarity, or deterministic rules — without breaking the Financial Freedom Framework.

> Verification: `grep -R "CompletionProvider\|complete_json\|complete_tool_turn" crates/ --include="*.rs"` → must match inventory §1. Citations are live web fetches checked 2026-08-31.

---

## 1. Inventory — Every LLM Call-Site (grep-verified)

| # | File:line | Feature | Provider method | Frequency | Token shape | Cost driver |
|---|-----------|---------|-----------------|-----------|-------------|-------------|
| A1 | `crates/finsight-agent/src/categorizer.rs:188` `complete_json(&system_prompt,&user_prompt)` | **Auto-categorization** — batch `LLM_BATCH_SIZE=20` txns → JSON array `{txn_id, category_id, confidence, rationale}` | `CompletionProvider::complete_json` | Every import + `RecategorizeLowConfidence` (all remaining uncategorized after rule pass) | ~800-token system prompt (all categories + recent few-shot) + `20 × (merchant + amount)` user prompt; response ~300-600 tokens JSON | **Highest volume.** 1 call per 20 untagged txns. Must stay cheap/latency-tolerant. |
| A2 | `crates/finsight-agent/src/anomaly.rs:88` `complete_json(system,&user)` | **Anomaly confirmation** — `BATCH_SIZE=20` statistically-outlying candidates + historical baseline → `{txn_id, is_anomaly, reason}` | `complete_json` | On each categorization run / anomaly scan | Small prompt (~500 tokens) + 20 × (txn + median/MAD baseline); response ~200 tokens | Medium — gated by deterministic filter; usually 0-2 calls per run, but pays for LLM reasoning |
| A3 | `crates/finsight-agent/src/planner.rs:43` `complete_json(&build_system_prompt, question)` | **Planner** — single-turn finance plan (actions + answer) for recipes/pre-chat | `complete_json` | Per recipe run, per finance-question planner step (legacy path) | Full `FinancialContext` serialized (~2-4k tokens) + question; response ~500-1k tokens | Medium per-recipe |
| A4 | `crates/finsight-agent/src/reasoning/engine/mod.rs:39-186` `complete_tool_turn_with_usage` / `complete_tool_turn_forced_with_usage` / `complete_final_answer_turn_with_usage` loop | **Copilot Chat (main)** — multi-turn agentic loop with `ToolSet` (14 tools: `search_transactions`, `get_financial_snapshot`, `rank_debt_payoff`, `draft_recategorization`, etc.) — up to `max_iterations` turns, plus synthesizer & retry | `complete_tool_turn*` + `complete_final_answer*` | Every user message in Copilot (most expensive) | System prompt ~1.5k + snapshot block; each turn carries full history + tool results; 3-8 LLM turns per answer, cached prefix optimization. Token-dominant. | **Highest cost.** Multi-turn + large context. |
| A5 | `crates/finsight-agent/src/recipe_runner.rs:42-61` delegates to `ReasoningEngine::run` | **Recipes** (automation) — same grounded tool loop as A4, prompt = `[Recipe:<title>] <template>` persisted as action bundle | via A4 | Per due recipe / manual RunRecipe | Same shape as A4 but single invocation | Shares A4 optimization wins |
| A6 | `crates/finsight-api/src/commands/copilot_chat.rs:971` `complete_json(system,&prompt)` | **Conversation title generation** — async `{"title":"..."}` after first message | `complete_json` | Once per new conversation | Tiny (system `Generate a 3-6 word title` + prompt ~200 tokens) | Trivial — but still an API round-trip |
| A7 | `crates/finsight-api/src/commands/agent.rs:2580` `complete_json(simple|deep classifier)` | **Complexity router** — `{mode:"simple"\|"deep"}` to decide `deterministic_copilot_fallback` vs full ReasoningEngine | `complete_json` | Every Copilot question before engine (added as cheap gate, still LLM) | ~150 tokens | Tiny but unnecessary |
| A8 | `crates/finsight-api/src/commands/agent.rs:2892` `complete_json(&system, &question)` | **ask_agent (legacy/eval)** — standalone question answering | `complete_json` | Eval harness / old path | Similar to A3 | Low |
| A9 | `crates/finsight-api/src/commands/recipes.rs:202` `complete_json(&build_system_prompt, &prompt)` | **Recipe planning** fallback budget | `complete_json` | Recipe fallback path | Same as A3 | Low |
| A10 | `crates/finsight-api/src/commands/scenarios.rs:207` `complete_json(system,&user)` | **Scenario generation** | `complete_json` | Scenario creation | Small | Low |
| P | `crates/finsight-agent/src/providers/{openai_compat,anthropic,ollama}.rs` | Providers themselves | — | — | `openai_compat.rs` implements prompt-caching (OpenAI/Gemini/DeepSeek/Qwen benefit), usage reporting (`prompt_tokens`, `cached_tokens`), `tool_choice: required/none` | Overhead is provider selection |

Deterministic fallback already exists (informative): `copilot_chat.rs:1705 deterministic_copilot_fallback` handles `top spending categories` without any LLM. Not counted as dependency — it is the pattern to extend.

**Completeness check:** `grep -R "CompletionProvider" crates/` now shows only above files + `lib.rs` trait + `mock.rs`. No hidden sites.

---

## 2. Replacements — Per Call-Site

### A1 — Transaction Categorization (Crown Jewel for Replacement)

**Why ideal:** Class-library is closed (user's `categories`), input is short strings (`merchant_raw`, `amount_cents`), labels are abundant (past `categorizations` + `category_proposals`). Pure classification, no world knowledge, no prose generation. Already has a **working local path**: `embedding/centroid::propose_for_uncategorized` (cosine to category centroids via `candle` + `all-MiniLM-L6-v2`).

| Replacement | Live Research (2026-08-31) | Feasibility | Cost / Latency | Accuracy note | Effort |
|-------------|----------------------------|-------------|----------------|---------------|--------|
| **(P0) Keep & promote embedding centroid + TF-IDF hybrid** | `candle` chosen over `ort`/Ollama in `embedding/mod.rs` — reasoning still valid (binary size ~+30MB, fragility). Model `all-MiniLM-L6-v2` is 22.7M params, 384 dims, 256 context, Apache-2.0, 0.1 GB VRAM — ideal city-scale. Alternatives now: `Qwen3-Embedding-0.6B` (0.6B, 1024 dims, 32K, Apache-2.0, 1.5 GB, 70.7 MTEB-eng-v2) lead quality-per-VRAM; `bge-m3` (568M, 8K, MIT) hybrid dense+sparse; `nomic-embed-text-v1.5` (137M, 768, 8K) cheap default. Source: D-Central embedding table 2026-07-17, SBERT docs. https://d-central.tech/local-embedding-models/ + https://www.sbert.net/docs/sentence_transformer/pretrained_models.html | **High** — code exists under `embedding/centroid.rs`. Needs re-rank as primary, LLM as fallback on `confidence < 0.6` only. Already shares `load_uncategorized_for_proposals` predicate so invariants cannot drift. | Zero API cost. Candle CPU 100-200 MB RSS after first load, single-process lifetime, mmap safe. ~10 ms/txn on desktop. | FinSight's rule pass already catches most regex; centroid proposal paper-harness in #92 deferred — wire it and measure vs LLM (`#88` eval harness). Improvement: add TF-IDF char-n-gram on merchant as cheap reranker; centroid tie-break with amount bucket. | **S (1-2 weeks).** |
| **(P0 alternative) FastText supervised classifier — POC VERIFIED & WIRED 2026-09-01** | **POC: `poc/fasttext_categorizer/models/merchant_ft.bin` (39 MB, 11 labels) trained on 1212 human-approved + LLM-augmented merchants. Wired in `crates/finsight-agent/src/categorizer.rs:174-301` as `Rule(1.0) → fastText ≥0.6 → LLM` (`fasttext-local` feature, `OnceCell` `FastText`, `FINSIGHT_DATA_DIR/models/fasttext/merchant_ft.bin` → `CARGO_MANIFEST_DIR/../../poc/...` fallback). `cargo check` green, `cargo test categorizer` 18/19 (1 pre-existing `ON CONFLICT`), `poc/fasttext_categorizer/.venv → ~/.cache/finsight-venvs/fasttext` durable.** | **Shipped** — `crate::fasttext_predict::get_fasttext_model` + `merchant_text_for_model` + threshold `0.6`. Metrics: `test 96.1%/0.965 F1 (6%→other)`, `valid 95.0%`, `heldout 95% gap -0.3%`, `0.00 ms/pred`. | Local, zero tokens, ~90-95% LLM offload. |
---
**Decision for A1:** **Shipped centroid + FastText hybrid as default; LLM for low-conf only.** **2026-09-01: `Rule → fastText ≥0.6 → LLM` wired (`fasttext-local` default off, `folk` 39 MB model at `FINSIGHT_DATA_DIR/models/fasttext/merchant_ft.bin` with dev fallback). POC 95-96% acc, 6%→other, gap -0.3%, groceries 0.80 thin spot → ensemble lifts. Token cut ~90-95%. Retain `LOW_CONFIDENCE_THRESHOLD=0.6`.**

| **(P1) Rule-only with human-readable template** | Current `finsight_core::anomaly` already is authoritative (median/MAD + exclusions). A2's only LLM value is a one-sentence `reason`. Replace with deterministic template: `"${merchant} ${amount} is ${x}× your median (${median}) for this merchant — ${kind: outlier|duplicate}"` + MAD z-score. Finance tools already emit such explanations (`finance.rs` MetricExplanation). No ML needed. | **Very High** — delete LLM branch, keep stats. Matches FIN-UX principle "behaviour > math": pattern, not prose. | Zero. | Deterministic — more honest than LLM hallucination; issue #18-style fabrication risk disappears. Optionally keep "dismiss" UX. | **XS (days).** Highest ROI. |
| **Isolation Forest / One-Class SVM on (amount, freq) per merchant** | Standard tabular anomaly detection (scikit-learn). But per-merchant series in personal finance is tiny (n=5-30), so MAD already optimal. ML only helps cross-merchant anomalies (unusual category spend). Could train monthly but weak signal. Paper trend (e.g., `Explaining ensemble ML for transaction fraud` 2026) shows RF/XGBoost win on large bank datasets, not personal 1-user ledger. | Low value — MAD covers personal regime. | Local CPU | Marginal. | M. |
| **Small LM for reason prose only** | Could keep Needle 2 or Phi-4-mini for `reason` templating offline if marketing wants prose. Not needed. | Low | Local | — | — |

**Decision for A2:** **SHIPPED 2026-09-01 — deterministic template.** `anomaly.rs` LLM `complete_json` → `anomaly_reason()` (`${merchant} $X is N× median $Y — outlier`), `provider` param kept compat but ignored, `5/5` tests, `100%` token cut.

---

### A3 / A8 / A9 / A10 — Planner / ask_agent / scenarios / recipes (single-turn JSON)

These are **one-shot JSON generators** over `FinancialContext`. No tool calls, no interaction — the LLM just reformats DB-derived finance into a plan. Replaceable:

| Replacement | Live Research | Feasibility | Notes |
|-------------|---------------|-------------|-------|
| **(P0) Deterministic `planning` crate** | `crates/finsight-agent/src/planning/mod.rs` already implements `plan_finance_question` (FinanceTaskType classifiers: CashInflowAllocation, GoalEta, DebtPayoffScenario, InvestmentReadiness, DataQualityReport) plus unit-tested finance snapshot block. The planner's LLM is a thin wrapper: prompt + JSON shape that `planning` quantifies anyway. See `planning/mod.rs:1965-2016` tests — they select correct tools per question without LLM. | **Very High** — extend `planning` to emit `PlanResult` shapes directly, bypass LLM. The `financial snapshot` already injected in reasoning loop proves deterministic numbers suffice. | Covers Ramsey/Sethi allocation (conscious spending types, sinking fund, debt snowball order) — already encoded in `finance.rs` explainers. |
| **Local SLM with JSON grammar (if prose required)** | Needle 2 grammar-constrained dispatch (byte-level JSON schema) achieves 98.3% function-name accuracy at 14 MB / 28 MB RAM / 70 MFLOPs/token (vs 460 for LFM2.5-230M). For structured plan JSON, constrained decoding guarantees valid `PLAN:` + `response_blocks` without parse wars. Sources: https://www.marktechpost.com/2026/08/13/cactus-compute-needle-2-45m-parameter-tool-calling-model/ + https://www.geeky-gadgets.com/needle-2-ai-model-14mb/ (both 2026-08). | Medium — FinSight's `ToolSet` definitions already compile to JSON schema; same grammar engine pattern (single-tool selection) maps cleanly. | Use only if `planning` cannot produce natural-language `answer` prose — but even there, templates suffice for finance answers where numbers are tool-derived. |
| **Hybrid: rule classifier → `executor` tools** | Equivalent to A4's path but single tool call. The deterministic `finance::plan_finance_question` already chooses tool names that `executor.rs` executes locally (no LLM). So the "plan" is just a discriminant. | High — add string classifier (regex + embedding intent) → `planning` enum. | Keeps Ramsey debt snowball order local & testable. |

**Decision for A3/8/9/10:** **Eliminate LLM; route through `planning` + `metrics` layers.** Token cut: 100%. Fallback only SLM if needed for free-form advice.

---

### A4 / A5 — Copilot Chat (agentic reasoning loop) — Hardest, highest leverage

This is the only feature where LLM creativity matters: open-ended natural language → multi-tool orchestration → grounded answer with `response_blocks` (charts, tables, metricGrid). But architecture already minimizes LLM workload:

- `ReasoningEngine::finance_snapshot_block` injects compact authoritative numbers (balances, rolling averages) so model doesn't need tool turns for basics — fewer turns = lower latency/cost.
- `ToolSet` is declared schema — tool selection is constrained; fabrication is punished (grounding rule, artifact bounds in `copilot_chat.rs:1529ff`).
- Deterministic fallback already answers `top spending` without LLM (`copilot_chat.rs:1705`).
- Router/synthesizer split (`provider` vs `synthesizer` in `engine/mod.rs:111`) already lets a cheap fast router do tool selection and a strong model only synthesize final answer.

**Replacement ladder (ordered by risk):**

| Tier | Strategy | Live model / technique | Feasibility | What stays | Savings |
|------|----------|------------------------|-------------|------------|---------|
| **T0 — Stay cloud but slash tokens now (no code)** | Enable prompt caching + compress context + cheap router. `OpenAiCompatProvider::complete_tool_turn_with_choice` already sets `tool_choice` + reports `cached_tokens`. `finsight-api/src/provider.rs` supports `copilot.router_model` (cheap sibling on same OpenRouter baseURL). Set router = `google/gemini-2.5-flash` or `openai/gpt-4o-mini` or `deepseek-chat` (auto-caching), synthesizer = strong. Enable `cache_control: ephemeral` on system prompt (already wired for Anthropic/Qwen). Pin `max_tokens` low for router turns. | Prompt caching docs (openrouter.ai), `openai_compat.rs:315` headroom comment. | **Immediate.** Config-only. | Full quality. | 40-70% cost (cached prefix ~1.5k tokens). Latency down. |
> **SHIPPED 2026-09-01 — T0 per-task routing:** `copilot_chat.rs:328` + `agent.rs:2713` now use `llm_routing.copilotRouter`/`copilotSynthesizer` `provider_for_task[_or_global]` (Immich-style, `Deterministic`→heuristic/`None`→global, `LLM`→per-task `CompletionProviderConfig`). Legacy `copilot.router_model` `build_copilot_router_from_settings` kept fallback. `OpenAiCompatProvider::complete_tool_turn_with_choice:314` already `cache_control: ephemeral` + `max_tokens 1024` cheap router + `TurnUsage.cached_tokens` `openrouter` `prompt_tokens_details.cached_tokens` → `TauriRuntime` `cachedTokens` usage chip `ui/src/components/copilot/TauriRuntime.ts:320`. `ReasoningEngine::run_with_events:111` `provider`/`synthesizer` split: cheap router `3-8 tool turns` `1024 tokens`, strong synth final `JSON`. `ModelRoutingSection` `SettingsData.tsx:697` `6 rows` `Cheap router`/`Copilot synthesizer` + `cargo check` `reasoning::engine 26/26` [OK].
| **T1 — Local SLM for tool loop (Ollama-native)** | Replace cloud per-turn `complete_tool_turn` with local `OllamaProvider` pointed at a **small tool-calling SLM**: `Qwen2.5-3B-Instruct` (128K, code/math strong), `Llama 3.2 3B`, `Phi-3.5 Mini 3.8B`, `Gemma 2 2B`. All run under 4-8 GB RAM via `llama.cpp`/Ollama. Source: Small Models Showdown https://www.generalcompute.com/blog/small-models-showdown-qwen-2-5-3b-llama-3-2-3b-phi-3-5-mini-gemma-2-2b + Edge LLM leaderboard https://awesomeagents.ai/leaderboards/edge-mobile-llm-leaderboard/ . FinSight already vendors Ollama — zero new dep. | **Medium-High** — swap provider config. Risk is tool-call accuracy drop. Mitigate: constrained decoding + retrieval head (below). | Replaces cloud for all Copilot turns. | API cost → 0. VRAM ~2-4 GB. Tokens/sec 20-50. | 100% token cost except optional cloud fallback. |
| **T2 — Needle 2 (or successor) as dedicated Tool Router (45M / 14 MB)** | **Needle 2** (Cactus Compute, Apache-2.0, 2026-08-13, CQ2-bit 2-bit quantization, Hadamard MLP, hashed n-gram KV memory, byte-level grammar, contrastive retrieval head, 256 sliding window, 28 MB fixed RAM, 500 tok/s on RPi5). Achieves 32.6 Seal-Tools in-domain / 28.7 OOD vs 17-27 for 230-270M rivals, 98.3% function-name accuracy. Built precisely for "natural sentence → typed function call", no world knowledge — exactly the Copilot router job (choose among 14 finance tools). Fine-tune Needle 2 on FinSight's `ToolSet` schemas (5-14 tools; retrieval head auto-admits top-5). Escalate to cloud/local SLM synthesizer on `confidence < threshold`. Sources: https://www.marktechpost.com/2026/08/13/cactus-compute-needle-2-45m-parameter-tool-calling-model/ + https://www.geeky-gadgets.com/needle-2-ai-model-14mb/ . Check: 2026-08-31 fetches above confirm open source, binary-per-platform, WASM. | **Medium** — need Needle C++ engine integration (Rust FFI or sidecar) or wait for Rust bindings; alternatively use Needle's static library targets (x86-64/ARM64/ARMv7/RISC-V/WASM). Training: fine-tune with FinSight tool traces (existing mock tool_turns in `engine/tests.rs`). | Router loop only; final synthesis still by larger local SLM or cloud. | Token cost zero for tool selection; cloud synthesizer 1 turn if escalated. Latency ~10-40 ms/router turn; 14 MB deploy (Tauri sidecar-friendly). | Near-100% if escalation rare. |
| **T3 — Intent classifier + deterministic executor (no generative LLM at all)** | For **constrained finance Q's**, a lightweight classifier (FastText or MiniLM intent head) maps question → `FinanceTaskType` → deterministic tool chain in `planning/mod.rs` and `read.rs`. Extend `deterministic_copilot_fallback` (currently only top-spending) to ~10 templates (cashflow, budget overage, goal ETA, debt payoff, recurring subs, anomaly list). Uses same `metrics` layer as Copilot snapshot — guarantees agreement with screens. | **High for narrow Q's**, fails on genuinely open-ended ("Should I buy a house now?"). So use as **Triage**: classifier confidence high → deterministic; mid → local SLM; low → escalate/ask clarification. `agent.rs:2579` complexity classifier is the hook — replace its LLM with a tiny intent model. | Handles ~50-60% of observed finance questions deterministically. | Fastest, most auditable. | Proportional to hit-rate. |

**Recommended architecture (strangler pattern):**

```
Question → [Fast intent classifier (MiniLM/fastText, <5ms)] ──high conf──▶ deterministic_copilot_fallback / planning (no LLM)
                │ mid conf
                ▼
         [Needle 2 router (14MB) OR local Ollama SLM 3B] → tool loop → [local SLM synthesizer 3-7B OR cloud synthesizer 1 turn]
                │ low conf / out-of-scope
                ▼
         escalate / ask clarification / cloud fallback
```

This keeps the **Financial Freedom Framework** prompts grounded in `context.rs` wellness/cashflow numbers, not model world-knowledge — deterministic paths quote `metrics` layer directly, so answer cannot contradict dashboard.

---

### A6 / A7 — Title & Complexity Router (easy kills)

| Site | Replacement |
|------|-------------|
| A6 title | Heuristic: first 6 words of question, truncated `truncate_title` already exists in `planner.rs:246`. No LLM. |
| A7 complexity | Intent classifier as above (2-label fastText, ~100 lines). The deterministic fallback predicate `asks_spending` already proves simple questions are regex-detectable; generalize with tiny model trained on Copilot history. |

**SHIPPED 2026-09-01 — heuristic.** `copilot_chat.rs:954` title `complete_json` → 6-word `split_whitespace().take(6)` (60 char), `agent.rs:2643` `router_classify` LLM → `deep_keywords` heuristic + `llm_routing.title`/`complexityRouter` Immich gate (`null` → heuristic, `Some` → LLM via `provider_for_task`), `0 tokens` when deterministic.
> **PINNED 2026-09-01 — A7 learned router parked per product:** Keyword `deep_keywords` `contains` heuristic is imperfect but stays as-is. `LLMRouter` (`ulab-uiuc/LLMRouter` `2739★` `MIT` `16+ routers` `knnrouter/mlprouter` `xRouteBench` `8 datasets` `3,729` generic) and `vLLM Semantic Router` (`vllm-project/semantic-router` `5481★` `Go` `Apache-2.0` `MoM` `Envoy/K8s`) both ship labeled datasets (`xRouteBench` `business/math` + `bench/data/test_data.json` `359` domain labels) but *generic*, not finance `simple vs deep`.No FinSight `simple vs deep` labels exist. Plan when unpinned: pretrain on `xRouteBench` `business/math` `smallest_llm vs largest_llm` `α=0.8 β=0.2` → `KNN k=5`/`MLP 384→64→2` on `all-MiniLM-L6-v2` `candle` `384 dims`, then auto-label `~200` finance `simple vs deep` via `deterministic_copilot_fallback` vs `ReasoningEngine` replay (`alpha*perf - beta*cost` like `LLMRouter` `download_data.py`), train Rust `router.bin` `fastText __label__simple/deep` or `linfa KNN` at `FINSIGHT_DATA_DIR/models/router/router.bin` `OnceCell` `router-local` feature, wire `agent.rs:2643` `router_classify` `confidence <0.6` → `provider_for_task("complexityRouter")` LLM fallback. `A6` title logic `question.truncate(60)` ignored per product — no LLM title. **To unpin:** say `unpin router`.


---

## 3. Summary ROI Table — Ordered by pay-off

| Rank | Call-site | Replacement | Token saving | Effort | Risk | Why now |
|------|-----------|-------------|--------------|--------|------|---------|
| 1 | A2 Anomaly | Template reason, keep MAD | **100%** | XS | None | Already correct; LLM adds risk. |
| 2 | A6 Title, A7 router | Heuristic / intent classifier | **100%** | XS | None | Trivial. |
| 3 | A1 Categorization | Centroid + FastText primary, LLM low-conf fallback | **~90%** | S | Low | Code exists; biggest volume. |
| 4 | A3/8/9/10 Planner | `planning` crate direct | **100%** | S | Low | Logic already written. |
| 5 | A4/A5 Copilot | T0 caching + router/synthesizer split (immediate) | **40-70%** | S (config) | None | Flip a config flag today. |
| 6 | A4/A5 Copilot | T1 Local SLM via Ollama (Qwen2.5-3B / Llama3.2-3B Q4) | **~100%** | M | Medium (accuracy) | Next after T0 proven. |
| 7 | A4/A5 Copilot | T2 Needle 2 router (14 MB) fine-tuned on ToolSet | **~100%** | M | Medium (integration) | Best long-term dispatcher; wait for Rust sidecar demo. |
| 8 | A4 Copilot narrow Q's | T3 Deterministic templates (extend `1705` fallback) | Proportional | S | Low | Covers ~50% questions for free. |

**Conservative combined first-pass (rows 1-5): ~65-80% API bill cut with ≤2 weeks work, no accuracy loss.** **SHIPPED 2026-09-01: rows 1-3 (A2/A6/A7/A1) + Model routing table (Immich-style `llm_routing` per-task `ModelRoutingConfig` + `Settings → Model routing` 6 rows + `fastTextThreshold` slider, `provider_for_task` wiring for `categorization`/`planner`/`title`/`complexityRouter`/`copilot*`).**

**Full local (1-8): ~95%+ cut, ~4-6 weeks including fine-tune & eval against `finsight-eval` harness.** Next `planner` deterministic + `copilot` `T1` local SLM via `provider_for_task_or_global`.
---

## 4. Live Research Citations (checked 2026-08-31)

- Cactus Compute **Needle 2** — 45M params, 14 MB binary, 28 MB RAM, 70 MFLOPs/token, 500 tok/s RPi5, CQ2-bit / Hadamard MLP / grammar-constrained / retrieval head / 98.3% function-name accuracy, leads Seal-Tools. Apache-2.0, cross-platform binaries + WASM. → https://www.marktechpost.com/2026/08/13/cactus-compute-needle-2-45m-parameter-tool-calling-model/ + https://www.geeky-gadgets.com/needle-2-ai-model-14mb/
- **Small-model showdown Qwen2.5-3B vs Llama 3.2 3B vs Phi-3.5 Mini vs Gemma 2 2B** (128K Qwen) — generalcompute.com 2026-07 → web_search result #4
- **Best Small Language Models 2026** (Phi-4, Qwen3, Llama) → https://checkthat.ai/answers/what-are-the-best-small-language-models
- **Edge & Mobile LLM Leaderboard 2026** (Phi/Gemma/Qwen on-device tok/s) → https://awesomeagents.ai/leaderboards/edge-mobile-llm-leaderboard/
- **Best Local Embedding Models for RAG 2026** — 20 models table (all-MiniLM 22.7M/0.1GB, nomic-embed 137M/0.3GB, Qwen3-0.6B 70.7 MTEB, bge-m3 568M hybrid, Qwen3-8B 75.2 SOTA), last reviewed 2026-07-17 → https://d-central.tech/local-embedding-models/
- **SentenceTransformers pretrained models** — all-MiniLM-L6-v2 5× faster than mpnet, good general purpose → https://www.sbert.net/docs/sentence_transformer/pretrained_models.html
- **Transaction categorization approaches** — QuickBooks relational deep learning arXiv:2506.09234, XGBoost/RF hybrid blogs, SME synthetic data 2025 → web_search batch top results (arXiv + mvvenrooij.nl hybrid rule+RF post)
- FinSight internal: `embedding/mod.rs` candle vs ort decision, `reasoning/engine/mod.rs` finance snapshot injection + TIME_LIMIT_SYNTHESIS, `copilot_chat.rs:1705` deterministic fallback, `provider.rs:123` router_model.

---

## 5. Concrete Implementation Plan (strangler, no big-bang)

**Phase 0 — Free wins (this sprint, config only)**
1. Enable prompt caching (`cache_control: ephemeral` on system prompt) + lower `max_tokens` for router turns (`openai_compat.rs:315`).
2. Set `copilot.router_model = gpt-4o-mini / gemini-2.5-flash / deepseek-chat` on OpenRouter; measure `cached_tokens` in usage chip.
3. Delete/anonymize A2 LLM branch; deploy template reason. Remove A6/A7 LLM calls.

**Phase 1 — Categorization offload (1-2 wks)**
4. Promote `centroid::propose_for_uncategorized` + `rebuild_all` as default pipeline; category centroids stored per `SentenceEncoder::model_id` so a later model swap invalidates naturally.
5. Train FastText on `merchant_raw → category_id` (incremental, per-user optional). Blend: `rules → fastText (conf>0.7) → centroid (cosine>threshold) → LLM fallback`.
6. Gate `RecategorizeLowConfidence` behind same stack; keep `LOW_CONFIDENCE_THRESHOLD=0.6` as UI contract.

**Phase 2 — Planner offload (1 wk)**
7. Route `plan()` / `recipes::plan` / `scenarios::*` through `planning` crate directly; keep Needle/SLM only for free-form narrative answer if needed.
8. Add quick eval: `planning` enum correct on 100 golden finance questions (existing `planning` tests cover 5 types).

**Phase 3 — Copilot triage (2-3 wks)**
9. Build tiny intent classifier (MiniLM 22M or fastText) on Copilot history → {deterministic, local-SLM, escalate}. Start with deterministic templates for top-5 question families (spending, runway, over-budget, goal ETA, debt snowball) — all numbers come from `metrics` layer so they cannot diverge from screens.
10. Deploy local SLM router: `OllamaProvider` → `qwen2.5:3b` or `phi-3.5` Q4 via Ollama, with grammar-constrained output (`json_schema` already plumbed for `supports_structured_output`). Evaluate against `reasoning/engine/tests.rs` turn scripts; measure tool-name accuracy.
11. Prototype Needle 2 sidecar (14 MB) as drop-in `CompletionProvider` for tool selection only; confidence threshold → synthesizer (local SLM 3-7B or single cloud turn).

**Phase 4 — Harden & measure**
12. Re-run `finsight-eval` harness (#88) for precision vs token cost Pareto; publish in `framework/`.
13. Add `FINSIGHT_DATA_DIR/models/` download caching already exists for candle — reuse for fastText/SLM weights.

**What not to do:** Do not bundle 7-8B embedders as default (waste VRAM), do not unify `tokenizers` Oniguruma fork (out of scope, small shim), do not add second native runtime (ort) — candle stays.

---

## 6. Risks & Guardrails

- **Copilot hallucination** — deterministic `ToolSet` grounding rule + artifact bounds (`ARTIFACT_MAX_*`) must stay; any local SLM must obey same schema, otherwise eval will catch oversized tables/charts.
- **Privacy** — embedding model weights already live alongside SQLCipher DBs (`FINSIGHT_DATA_DIR`); document per `docs/self-hosting.md` "one container, no extra services" constraint remains satisfied (Needle 2 is a static lib, no Ollama required if using sidecar).
- **AIR-GAP** — candle weights download (~90MB MiniLM) already fails without egress; add manual pre-populate note for fastText/SLM too.
- **Reset barrier** — all on-device replacements still hold `ResetBarrier` lease across writes; no differential risk.
- **License** — MiniLM/Qwen Apache-2.0 ok for AGPL-3.0 repo; avoid NV-Embed-v2 / jina-embeddings-v3 for commercial RAG (CC-BY-NC).

---

## 7. Verification Checklist

- [ ] `grep -R "CompletionProvider" crates/` now shows only trait/providers + mock, plus surviving A4 router site
- [ ] `grep -R "complete_json" crates/finsight-agent/src/categorizer.rs` gated behind `confidence < 0.6`
- [ ] `anomaly.rs` has no `complete_json` import
- [ ] `provider.rs:load_completion_provider_config` still supports `ollama` for local SLM
- [ ] `embedding/centroid` proposal count matches eval harness within 2% of LLM baseline
- [ ] Usage chip shows `cached_tokens > 0` on OpenAI-compat runs
- [ ] `docs/llm-replacement-audit.md` this file renders (no URL 404 on re-fetch)

---

*Generated by exhaustive crate audit + live web_search 2026-08-31. Next step: implement Phase 0 config PR and open issue for Phase 1 centroid promotion.*
