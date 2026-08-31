# Installation

FinSight ships as a single Docker image. You do not compile it on the server.

## Prerequisites

- Docker Engine and `docker compose` (bundled with recent Docker).
- Disk for the image (a few hundred MB) plus your data (`/data` grows with CSV history; generous is cheap).
- One of: Tailscale (recommended), a domain + Caddy, or just your LAN — see [Self-Hosting](/help/self-hosting).

## Quick start

From the repository root where `docker-compose.yml` lives:

```bash
git clone https://github.com/Koushik0901/FinSight.git
cd FinSight
docker compose up -d
docker compose logs -f finsight
```

The first run pulls `ghcr.io/koushik0901/finsight:latest` (multi-arch: AMD64 + ARM64) and creates a named volume `finsight-data` for `/data`. The container is `read_only`, `no-new-privileges`, `cap_drop: ALL`, only `/data` and a small `tmpfs` are writable.

To build this checkout instead (development or local patches):

```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build -d
```

Open `http://localhost:8674` on the host to smoke-test. Then put it behind HTTPS for normal use — browsers require a secure context for the PWA, offline cache, and share-target.

## Configuration

Copy `finsight.env.example` to `.env` beside `docker-compose.yml` when you need to pin a version or change ports:

| Variable | Default | Purpose |
|---|---|---|
| `FINSIGHT_IMAGE` | `ghcr.io/koushik0901/finsight:latest` | Image tag/digest to deploy |
| `FINSIGHT_HOST_PORT` | `8674` | Host port |
| `FINSIGHT_COOKIE_SECURE` | `1` | `Secure` on session cookies; set `0` only for bare-HTTP LAN testing |
| `FINSIGHT_PUBLIC_ORIGIN` | inferred | External HTTPS origin when proxy headers are insufficient |

Inside the container the server also respects `FINSIGHT_DATA_DIR` (`/data`), `FINSIGHT_UI_DIR` (`/app/ui/dist`), `FINSIGHT_PORT` (`8674`), and `RUST_LOG`.

## First account

The first visit to a fresh instance shows the setup wizard. The first account becomes the administrator and receives a **one-time recovery key**.

> Save the recovery key in a password manager or printed copy *immediately*. FinSight cannot reset it for you.

After sign-in the admin can add non-admin users at **Settings → Account → Manage users**. Each receives a separate encrypted database and its own recovery key — the admin must hand it over securely.

Passwords must be at least 10 characters. Login and recovery share a per-username throttle: five failures → 60-second cooldown (including unknown usernames). Sessions have a sliding 30-day lifetime and survive restarts.

## Updating

```bash
docker compose pull
docker compose up -d
```

For a pinned version, set `FINSIGHT_IMAGE` in `.env` to the release tag. Versioned GitHub Releases include a Compose file pinned to the same tag.

Back up `/data` before upgrading. Include `users.db`, `session.key`, and every `users/<uuid>/`.

Next: [First Launch](/getting-started/first-launch).
