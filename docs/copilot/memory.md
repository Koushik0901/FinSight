# Agent Memory

Small, reviewable facts the Copilot remembers so you do not have to repeat yourself.

## What is stored

`crates/finsight-core::repos/agent_memory` holds rows in your encrypted database:

- User preferences it has observed (e.g. “User prefers grocery budget at $600”).
- Clarifications it has already asked and received answers for.
- Context that improves grounding without bloating the prompt.

Memories are **per-user**, shown at **Insights → Memory**, and deletable there. Deleting one is immediate — the Copilot will not re-create it without fresh evidence.

## How memory is used

At context-build time, `build_context` selects memories relevant to the current question and includes them in the system prompt. Irrelevant memories are not sent — they do not widen the prompt or leak across tasks.

The model is instructed to cite figures, not memories, as the authority. A memory explains a preference; the ledger explains a total.

## Controls

- **Review** — Insights → Memory lists everything stored.
- **Delete** — per-row delete; no bulk “forget me” that erases unknowns.
- **Opt-out** — do not store new memories by simply deleting as they appear; there is no hidden retention.

Memory exists to reduce repetition, not to profile. Keep it small and it stays useful.

See also: [How the Copilot Works](/copilot/how-it-works), [Privacy](/copilot/privacy).
