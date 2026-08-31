# First Launch

After `docker compose up -d`, open `http://localhost:8674` on the Docker host.

## The setup wizard

1. Pick a username and a password (≥10 characters).
2. The server generates a **recovery key** — a printable, high-entropy secret that wraps your database key alongside your password.
3. The wizard shows the key once. Save it before continuing — it is the only way back after a lost password. Recovery resets the password, rotates the key, and revokes existing sessions.

The first account is the sole administrator. Use **Settings → Account → Manage users** to add household members. Each gets an isolated `users/<uuid>/data.sqlcipher` and a one-time key you must deliver securely. Deleting a user removes their entire `users/<uuid>/` directory and revokes their sessions.

## Cookies and HTTPS

`FINSIGHT_COOKIE_SECURE=1` is the default (and the correct value behind HTTPS). The cookie is `HttpOnly`, `SameSite=Lax`, 30-day sliding lifetime, persisted via `/data/session.key` — logout and recovery revoke it.

For a local smoke test `http://localhost:8674` works without override: browsers treat `localhost` as a secure context.

For bare `http://<lan-ip>:8674` from another device, temporarily set:

```yaml
environment:
  FINSIGHT_COOKIE_SECURE: "0"  # LAN testing only — revert to 1 before HTTPS
```

Revert to `1` before putting FinSight behind a proxy. Never leave `0` on anything reachable beyond your LAN.

## What needs a secure context

Without HTTPS (or `localhost`), three features are quietly inert — no error, they just do not happen:

| Feature | Without HTTPS |
|---|---|
| Installable PWA + offline shell | Service worker does not register |
| Encrypted offline cache | `crypto.subtle` unavailable → cache is not written at all; data is never stored in the clear |
| Share-target import, Web Push, badges | Depend on the worker above |

Browsing over `http://<lan-ip>:8674` still works as a plain web app; you just lose the installed experience. See [Self-Hosting](/help/self-hosting) for Tailscale, Caddy, and LAN recipes.

## Where to go next

- **Onboarding** walks you through accounts — manual or SimpleFIN — and an optional CSV import.
- **Settings → Data & backups** handles encrypted snapshots. For disaster recovery, also back up the whole `/data` volume so `users.db` and every per-user database stay together.

Next: [Onboarding](/getting-started/onboarding).
