# Settings

All user-controlled configuration — provider, data, and server account.

## Sections

- **Agent** — provider choice (Ollama / OpenAI-compatible / Anthropic), models, and the data-flow explanation shown before you save. Keys are per-user in `data.sqlcipher`.
- **Data & backups** — create encrypted per-user snapshots under `users/<uuid>/backups/` and restore them. For disaster recovery, also back up the whole `/data` volume (includes `users.db` + `session.key`).
- **Exports** — CSV export of transactions/reports; streamed from the server, no third party.
- **Account** — rename self, manage users (admin only), change password, recovery key rotation, and “sign out other devices” (revokes persisted sessions).
- **Appearance** — theme (system / light / dark) and density; persisted to `localStorage`.

## Provider keys

Changing a provider key re-wraps nothing and touches no other user — the key is a row in your encrypted database. Removing the key disables Copilot + auto-categorization immediately; deterministic heuristics remain.

## Admin

The first account is the administrator. At **Settings → Account → Manage users** it can create and delete non-admin users. Creation issues a one-time recovery key; deletion removes `users/<uuid>/` and revokes that user’s sessions.

See also: [Ollama](/configuration/ollama), [OpenAI-Compatible](/configuration/openai-compatible), [Anthropic](/configuration/anthropic).
