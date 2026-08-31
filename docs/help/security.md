# Security & Privacy

This page states what the implementation actually does. Claims are grounded in `crate/finsight-server::crypto.rs`, `sessions.rs`, `users.rs`, and `README`.

## Data at rest

- Each user’s ledger is a **SQLCipher-encrypted** database (`users/<uuid>/data.sqlcipher`) with a **random 32-byte key** per user.
- The key is stored only **wrapped**, twice:
  - `Wrap(Argon2id(password, kek_salt))` — Argon2id pinned at m=19456 KiB, t=2, p=1, id v19
  - `Wrap(recovery_key_bytes)` — the printable recovery key is the KEK directly (high-entropy, no KDF)
- Password verification uses a **separate** Argon2id PHC string (own salt) so the verifier cannot derive the KEK.
- `users.db` is **plain SQLite** — it must be readable before unwrapping. It holds usernames, Argon2id verifiers, wrapped keys, and hashed sessions — no financial records, plaintext passwords, or plaintext DB keys.
- Unwrapped keys exist **only in server memory**, single-flighted, evicted after 30 minutes idle unless an SSE client is attached.

## Data in transit & sessions

- Session cookies are `HttpOnly`, `SameSite=Lax`, sliding 30-day lifetime. `FINSIGHT_COOKIE_SECURE=1` adds `Secure` — required behind HTTPS (Tailscale/Caddy); set `0` only for bare-HTTP LAN tests.
- The cookie token is stored on the client; `users.db` holds its **hash** and a DB key wrapped by `/data/session.key` (persisted sessions survive restarts).
- Logout, recovery, deletion, and “sign out other devices” revoke the persisted rows.

## Attack surface

- **Docker**: `read_only: true`, `no-new-privileges:true`, `cap_drop: ALL`, only `/data` and a small `tmpfs` writable.
- **Browser cache**: seven-day, read-only IndexedDB query cache, encrypted when the PWA has a secure context; purged on logout. It is a convenience, not the vault.
- **When data leaves your server**: only on opt-in integrations — cloud AI (redacted context) and SimpleFIN (access URL exchange). See [Privacy & Local Data](/getting-started/privacy) for the table.

## What is not claimed

- Not end-to-end (browser↔server) encryption of ledger — the server must hold the unwrapped DB key in memory to serve your session.
- Not zero-knowledge browser cache — assume anyone with a usable session and device can read what the cache holds.
- `users.db` is not encrypted; protect `/data/session.key` and `/data` file permissions (owner-only on the host).

## Ops hardening

- Strong, unique passwords (≥10 chars enforced) + 60s throttle after 5 failures per username.
- Keep `FINSIGHT_COOKIE_SECURE=1` behind HTTPS; never leave `0` on an internet-reachable origin.
- Put the server behind HTTPS (see [Self-Hosting](/help/self-hosting)) and consider fail2ban for internet-facing deploys.

See also: [Data Storage](/configuration/data-storage), [Getting Started → Privacy](/getting-started/privacy).
