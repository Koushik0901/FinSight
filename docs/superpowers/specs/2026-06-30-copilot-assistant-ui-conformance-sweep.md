# Copilot assistant-ui Conformance Sweep

**Date:** 2026-06-30
**Status:** Implementation research recorded

## Research Checklist

- MCP docs reviewed: `tools/generative-ui`, `tools/tool-ui`, `ui/context-display`, `ui/scrollbar`, `ui/markdown`, `ui/quote`, `ui/sources`, `ui/streamdown`, `runtimes/custom/local-runtime`, `runtimes/concepts/threads`, and `runtimes/concepts/adapters`.
- MCP examples reviewed: `with-custom-thread-list`, `with-generative-ui`, and `with-resumable-stream`.
- npm latest versions checked during implementation:
  - `@assistant-ui/react@0.14.24`
  - `@assistant-ui/react-markdown@0.14.5`
  - `@assistant-ui/react-streamdown@0.3.5`
  - `assistant-stream@0.3.24`
- React 18 remains supported by the latest assistant-ui packages.

## Findings

- FinSight should keep `useLocalRuntime` because the model backend is a custom Rust/Tauri command surface, not AI SDK or assistant-cloud.
- Multi-thread ownership should move to `useRemoteThreadListRuntime` with a FinSight `RemoteThreadListAdapter` and thread-scoped `ThreadHistoryAdapter`.
- `RuntimeAdapterProvider`, `ThreadHistoryAdapter`, `ThreadListPrimitive`, `ThreadListItemPrimitive`, `AuiIf`, `ErrorPrimitive`, source/quote part types, and queueing are available in the installed `@assistant-ui/react` type surface.
- Registry UI components such as ToolFallback, Sources, Context Display, Scrollbar, and Quote are not exported as direct package components in this installed package. FinSight should use local equivalents that follow the primitive contracts and preserve the Plutus visual language.
- Generative UI should remain allowlist-only. The allowlist controls component names; props still need validation and must not flow into executable HTML or unsafe links.
- The resumable stream example is AI SDK/SSE oriented. FinSight should defer run resume until a durable Tauri run-status/reconnect command exists.

## Implementation Decisions

- Keep `copilot-stream-frame` as the Tauri event protocol.
- Add assistant-ui-native thread ownership through `useRemoteThreadListRuntime`.
- Use no-op history append/update/delete hooks where Rust streaming already persists the source user and final assistant messages, to avoid duplicate writes.
- Keep `ActionBundlePanel` as compatibility UI for existing draft action bundles while tool cards render through assistant-ui tool parts.
- Attempt to add Streamdown, but local package installation is blocked by the existing pnpm virtual-store mismatch and an npm postinstall failure in `axe-core` that expects `husky`. Until install is repaired, the renderer abstraction keeps `MarkdownTextPrimitive` as the buildable fallback.

