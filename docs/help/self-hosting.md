# Self-Hosting

> This page is a compact entry point. The full, authoritative guide is [`docs/self-hosting.md`](https://github.com/Koushik0901/FinSight/blob/main/docs/self-hosting.md) — read it top to bottom once, then return here for quick reference.

FinSight runs on hardware you control as a single Docker image.

## Quick start

```bash
git clone https://github.com/Koushik0901/FinSight.git
cd FinSight
docker compose up -d
docker compose logs -f finsight
# → http://localhost:8674  (smoke test)
```

First account → administrator; save the one-time recovery key.

## Choosing a way to reach it

| Recipe | Domain | Certs | Who can reach it |
|---|---|---|---|
| **A — Tailscale** (recommended) | `<host>.<tailnet>.ts.net` | Tailscale manages Let’s Encrypt | Only devices on your tailnet — nothing on the public internet |
| **B — Caddy + public domain** | `finsight.example.com` | Caddy manages Let’s Encrypt | Anyone on the internet (login-protected) |
| **C — LAN + mkcert** | `https://<lan-name>` | mkcert local CA | Only devices that trust your CA, on your LAN |

All three give you a **secure context** (HTTPS) — required for the PWA, encrypted offline cache, and share-target.

### Recipe A — Tailscale

```bash
tailscale serve --bg 8674
# → https://<device>.<tailnet>.ts.net
```

Enable MagicDNS + HTTPS certificates in the Tailscale admin console. Keep `FINSIGHT_COOKIE_SECURE=1`.

### Recipe B — Caddy

Point DNS at your host, forward 80/443, then front the app:

`Caddyfile`:
```caddyfile
finsight.example.com {
    reverse_proxy finsight:8674
}
```

Drop `ports:` from `finsight` — only Caddy faces outward — and keep `FINSIGHT_COOKIE_SECURE=1`.

### Recipe C — mkcert (LAN-only)

Generate a CA-trusted cert for a LAN hostname and front with Caddy/nginx. No VPN, no public port.

See the full guide for `mkcert` steps, brotli/Caddy notes, and PWA details.

## Upgrading

```bash
docker compose pull
docker compose up -d
```

Pin `FINSIGHT_IMAGE` in `.env` to a release tag for reproducible upgrades. Back up `/data` first (whole volume, so `users.db` + `session.key` + every per-user DB stay together).

## Details

The authoritative reference — Tailscale serve flags, Caddy HTTP/2 reasoning, LAN TLS, backup/compose split examples, and operational limits — lives in [`docs/self-hosting.md`](https://github.com/Koushik0901/FinSight/blob/main/docs/self-hosting.md).

See also: [Installation](/getting-started/installation), [First Launch](/getting-started/first-launch), [Security & Privacy](/help/security).
