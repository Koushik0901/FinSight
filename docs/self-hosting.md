# Self-hosting FinSight

FinSight's server mode (`finsight-server`) is an Immich-style self-hosted
service: it runs on hardware you control, stores each user's data in a
separate encrypted SQLCipher database under a single data directory, and
serves the same UI you'd get from the desktop app — reachable from your
phone, laptop, or anyone else's device on your account, without a cloud
subscription in between. This guide gets you from "nothing running" to a
working, installable, backed-up instance.

It assumes no prior Docker or reverse-proxy experience. Read it top to
bottom once; after that you'll only need the Quick Start and the recipe you
picked.

---

## 1. What you get / prerequisites

- **A Docker image, not a cloud service.** You run `finsight-server` yourself,
  on a machine you own — a home server, a NAS, a small VPS, or even a spare
  laptop that stays on. Nobody else operates it or has access to your data.
- **The server speaks plain HTTP on port `8674`.** It does not terminate TLS
  itself. If you want HTTPS (you almost always do — browsers require it for
  installable PWAs and service workers), you put a reverse proxy in front of
  it. Section 3–5 below cover three ways to do that, from easiest to most
  manual.

  Three client features need a **secure context** (HTTPS, or `localhost`) and
  are simply inert without one — no error, they just never happen:

  | Feature | Without HTTPS |
  |---|---|
  | Installable PWA + offline app shell | Service workers don't register |
  | Encrypted offline cache | `crypto.subtle` is unavailable, so the browser cache is **not written at all** — the app still works, it just re-fetches on every load instead of painting from cache. It never falls back to storing your financial data in the clear. |
  | Share-target import, Web Push, icon badges | Depend on the service worker above |

  Reaching FinSight over `http://<lan-ip>:8674` works, but you get a plain web
  app rather than an installed one. If you want the full experience, use one of
  the TLS recipes below.
- **All durable state lives under one directory, mounted as `/data` inside
  the container.** That includes `users.db` (the account registry, password
  verifiers, wrapped database keys, and hashed persistent sessions),
  `session.key` (which wraps persisted session keys), plus
  `users/<uuid>/data.sqlcipher`, backups, and import staging for each user.
  Unwrapped database keys exist only in server memory. Back up the whole
  directory; there is nothing else on the server to back up.
- **Prerequisites on the host:**
  - [Docker Engine](https://docs.docker.com/engine/install/) and the
    `docker compose` CLI (bundled with recent Docker installs; standalone
    `docker-compose` also works — swap the command spelling below).
  - Enough disk for the image (a few hundred MB) plus your data (transaction
    history compresses well; budget generously if you're importing years of
    CSVs).
  - One of: a [Tailscale](https://tailscale.com/) account (free tier is
    fine), a domain name you control, or just your home LAN — pick whichever
    recipe (Section 3, 4, or 5) matches how you want to reach the server.

---

## 2. Quick start (docker-compose)

From the repo root (where `docker-compose.yml` lives):

```bash
docker compose up -d
```

The first run pulls the public multi-architecture image from GitHub Container
Registry, then starts it in the background, publishing port `8674` and creating
a named volume (`finsight-data`) for `/data`. AMD64 PCs and ARM64 home servers
use the same Compose file.

To build the exact checkout you cloned instead (for development or local
patches), use the opt-in override:

```bash
docker compose -f docker-compose.yml -f docker-compose.build.yml up --build -d
```

Watch the logs until it's ready:

```bash
docker compose logs -f finsight
```

Then open `http://<host>:8674` in a browser (from the host itself, that's
`http://localhost:8674`). The first visit to a fresh instance shows the
**setup wizard**: pick an admin username and password, and the server
generates a **recovery key**.

> **Save the recovery key somewhere durable — a password manager, a printed
> copy in a drawer — the moment it's shown.** It's the only way back into an
> account if a password is lost; FinSight cannot reset it for you, by design
> (that's what makes the per-user encryption meaningful).

The first account is the only administrator. After signing in, it can use
**Settings → Account → Manage users** to add or delete non-admin users. Every
new user receives a separate SQLCipher database and a one-time recovery key;
the administrator must give that key to the user securely. Deleting a user
revokes their sessions and removes their entire `users/<uuid>/` directory.

Passwords must contain at least 10 characters. Login and recovery share a
per-username throttle: five consecutive failures trigger a 60-second cooldown,
with the same behavior for unknown usernames. Sessions use a sliding 30-day
lifetime and survive server restarts. Logout, recovery, account deletion, and
“sign out other devices” revoke the corresponding persisted sessions.

LLM API keys and SimpleFIN access URLs are stored inside each user's encrypted
database. They are not shared between users and do not depend on an OS
keychain or Linux Secret Service inside the Docker container.

**About `FINSIGHT_COOKIE_SECURE`:** `docker-compose.yml` ships with this set
to `"1"` (secure cookies, `Set-Cookie: ... Secure`), which requires the
browser to see the connection as HTTPS. That's correct once you're behind
one of the reverse-proxy recipes below. If you're kicking the tyres over
bare `http://` on your LAN with no proxy yet (Section 2 only, before you've
set up Section 3/4/5), the login cookie won't be accepted by the browser
until you either add a proxy or temporarily set it to `"0"` in
`docker-compose.yml`:

```yaml
environment:
  FINSIGHT_COOKIE_SECURE: "0"   # bare http, no reverse proxy — LAN testing only
```

Revert it to `"1"` once you're on HTTPS. Never leave it at `"0"` on anything
reachable outside your own LAN.

The container sets the supported runtime variables for you:

| Variable | Docker default | Purpose |
|---|---:|---|
| `FINSIGHT_DATA_DIR` | `/data` | Root for `users.db` and all per-user directories |
| `FINSIGHT_UI_DIR` | `/app/ui/dist` | Built SPA/PWA assets served by the server |
| `FINSIGHT_PORT` | `8674` | HTTP listen port |
| `FINSIGHT_COOKIE_SECURE` | `1` | Adds the `Secure` attribute to session cookies when exactly `1` |
| `RUST_LOG` | `info` | Server log filter |

Compose also reads these operator settings from an optional `.env` file beside
`docker-compose.yml` (copy `finsight.env.example` to `.env` as a starting
point):

| Variable | Compose default | Purpose |
|---|---:|---|
| `FINSIGHT_IMAGE` | `ghcr.io/koushik0901/finsight:latest` | Image tag or digest to deploy; pin a version for reproducible upgrades |
| `FINSIGHT_HOST_PORT` | `8674` | Port exposed on the Docker host |
| `FINSIGHT_COOKIE_SECURE` | `1` | Set to `0` only for temporary bare-HTTP LAN testing |
| `FINSIGHT_PUBLIC_ORIGIN` | inferred | External HTTPS origin when proxy headers are insufficient |

---

## 3. Recipe A — Tailscale (recommended)

The easiest path if you don't want to manage a domain, certificates, or
port-forwarding at all. [Tailscale](https://tailscale.com/) is a mesh VPN;
its `serve` feature also gets you a real, browser-trusted HTTPS certificate
for free via Let's Encrypt, scoped to your own private tailnet — no public
DNS record, no open port on your router.

1. Install Tailscale on the host running FinSight, and on every device
   you'll use to access it (phone, laptop, etc.), then sign them into the
   same tailnet: <https://tailscale.com/download>. Your own devices just
   sign in with your account; for another household member's phone,
   [invite them to your tailnet](https://tailscale.com/docs/features/sharing/how-to/invite-any-user)
   from the admin console's Users page — the free Personal plan currently
   includes six users, plenty for a household.
2. Enable [MagicDNS](https://tailscale.com/kb/1081/magicdns) and HTTPS
   certificates for your tailnet in the Tailscale admin console (Settings →
   enable "HTTPS Certificates"). If you forget, recent Tailscale versions
   notice during the next step and walk you through enabling both.
3. On the FinSight host, point Tailscale's built-in reverse proxy at the
   container:

   ```bash
   tailscale serve --bg 8674
   ```

   This terminates TLS with a certificate Tailscale manages and issues for
   you, and forwards to the FinSight container over plain HTTP on the
   loopback interface — Docker's own port publish (`8674:8674`) is what
   makes port `8674` reachable here. The `--bg` flag makes the mapping
   persistent: it survives closing the terminal and resumes after a reboot,
   until you remove it with `tailscale serve reset`. Without `--bg`, serve
   runs in the foreground and stops on Ctrl-C. (Guides written before
   Tailscale 1.52 show `tailscale serve https / <target>` — that older
   syntax no longer exists; use the form above.)

4. Reach the server from any device on the tailnet at
   `https://<device-name>.<tailnet-name>.ts.net`. Keep
   `FINSIGHT_COOKIE_SECURE: "1"` (the default) — Tailscale serve is real
   HTTPS, so secure cookies work correctly.
5. `tailscale serve status` shows the current mapping; `tailscale serve
   reset` tears it down if you want to reconfigure.

Nothing here opens a port on your home router or exposes the server to the
public internet — only devices logged into your tailnet can reach it. This
is the recipe to reach for if "just my household, from anywhere" is the
goal.

---

## 4. Recipe B — Public domain + Caddy

Use this if you want FinSight reachable at a real domain name from any
browser, with no VPN client required on the visiting device. This trades
convenience for a materially larger attack surface: the server becomes
reachable by anyone on the internet, protected only by your login. Only do
this if you understand and accept that trade-off — Recipe A (Tailscale) is
safer for the same "access from anywhere" goal.

If you go ahead: [Caddy](https://caddyserver.com/) is a reverse proxy that
requests and renews Let's Encrypt certificates automatically — you don't
touch certbot or manage renewal cron jobs.

1. Point your domain's DNS `A`/`AAAA` record at the host's public IP, and
   forward ports `80` and `443` from your router to the host (Caddy needs
   `80` briefly for the ACME HTTP challenge, then serves on `443`).
2. Add a `Caddyfile` next to `docker-compose.yml`:

   ```caddyfile
   finsight.example.com {
       reverse_proxy finsight:8674
   }
   ```

   Replace `finsight.example.com` with your real domain.

   You do **not** need to add a `encode gzip zstd` directive: FinSight already
   serves brotli-precompressed assets and compresses its JSON responses
   itself, and Caddy passes an upstream `Content-Encoding` through untouched.
   Adding compression here would only make Caddy try to re-compress bytes that
   are already compressed.

   What Caddy *does* add for free is **HTTP/2**. That matters here because the
   app is ~90 separate JavaScript chunks: over HTTP/1.1 a browser opens at most
   6 connections per origin and the rest queue, while HTTP/2 multiplexes them
   all down one. This is the main reason to front the server with a proxy even
   on a LAN.

3. Add a `caddy` service to `docker-compose.yml` on the same Docker network
   as `finsight`, and drop the host port mapping on `finsight` itself (Caddy
   is now the only thing facing outward):

   ```yaml
   services:
     finsight:
       image: ghcr.io/koushik0901/finsight:latest
       pull_policy: always
       restart: unless-stopped
       init: true
       # no `ports:` here — only Caddy is exposed externally
       volumes:
         - finsight-data:/data
       environment:
         FINSIGHT_COOKIE_SECURE: "1"

     caddy:
       image: caddy:2
       restart: unless-stopped
       ports:
         - "80:80"
         - "443:443"
       volumes:
         - ./Caddyfile:/etc/caddyfile:ro
         - caddy-data:/data
         - caddy-config:/config
       command: caddy run --config /etc/caddyfile --adapter caddyfile

   volumes:
     finsight-data:
     caddy-data:
     caddy-config:
   ```

4. `docker compose up -d`. Caddy fetches a certificate for your domain on
   first request and renews it automatically thereafter.
5. **Harden before exposing:** use a strong, unique admin password (this app
   holds financial data); keep the host OS and Docker patched; consider
   fail2ban or a similar tool watching for repeated failed logins; keep
   `FINSIGHT_COOKIE_SECURE: "1"` — Caddy's HTTPS makes secure cookies work
   correctly, and turning it off here would send your session cookie over
   the internet in plaintext.

---

## 5. Recipe C — LAN only + mkcert

No away-from-home access, no domain, no VPN — just a trusted HTTPS
certificate for devices on your home network. This is the right choice if
FinSight only ever needs to be reachable from inside your house, and you'd
rather not touch Tailscale or the public internet at all. It still needs
HTTPS, not just "leave `FINSIGHT_COOKIE_SECURE` off": browsers refuse to
register a PWA's service worker on a plain-`http://` origin (`localhost` is
the one exception, which is why Section 2's quick test works without a
proxy), so a LAN install still needs a trusted cert.

[`mkcert`](https://github.com/FiloSottile/mkcert) creates a local
certificate authority and installs its root into your OS/browser trust
store, so certs it issues are trusted without a "not secure" warning — but
only on devices where you've installed that root CA.

1. Install `mkcert` on the FinSight host and generate a root CA plus a
   certificate for the host's LAN hostname or IP:

   ```bash
   mkcert -install
   mkcert finsight.local 192.168.1.50   # your host's LAN hostname / IP
   ```

   This produces `finsight.local+1.pem` (cert) and `finsight.local+1-key.pem`
   (key) in the current directory.

2. Install the mkcert root CA (`mkcert -CAROOT` shows its location) on
   **every device** that will access FinSight — phone, laptop, etc.
   Instructions vary by OS; mkcert's README covers Android, iOS, macOS,
   Windows, and Linux. This step is what makes the certificate trusted
   instead of just self-signed; skipping it means every browser (and the
   PWA install prompt) will refuse the connection.
3. Front the container with Caddy (or nginx) using the mkcert cert instead
   of Let's Encrypt — a minimal `Caddyfile`:

   ```caddyfile
   finsight.local:443 {
       tls /certs/finsight.local+1.pem /certs/finsight.local+1-key.pem
       reverse_proxy finsight:8674
   }
   ```

   Mount both the cert files and this `Caddyfile` into the `caddy` service
   from Recipe B's compose snippet (swap the `image: caddy:2` command/volumes
   accordingly), and give your router a static DHCP lease or local DNS entry
   so `finsight.local` resolves on the LAN.
4. Reach the server at `https://finsight.local` from any device that trusted
   the mkcert root. `FINSIGHT_COOKIE_SECURE: "1"` works normally since this
   is genuine (locally-trusted) HTTPS.

---

## 6. Installing the app

Once you're on HTTPS (any of Sections 3–5) or `localhost`, FinSight is an
installable Progressive Web App — no app store, no separate binary.

- **Android / desktop Chrome (or Edge):** open the site, then either use the
  install icon in the address bar or the browser menu → "Install FinSight"
  / "Install app". It installs like a native app: its own window, its own
  icon in your app launcher/dock, no browser chrome.
- **iOS Safari:** open the site, tap the Share icon, then "Add to Home
  Screen". Safari doesn't expose a separate "install" affordance the way
  Chrome does — Add to Home Screen is the equivalent, and it produces a
  standalone app icon that launches full-screen.
- **Caveat — Safari's ~7-day storage eviction:** iOS Safari aggressively
  evicts site data (including the offline IndexedDB cache) after roughly a
  week of the PWA not being opened. FinSight's offline cache is designed as
  a convenience — it shows your last-synced balances and transactions when
  you're offline — never as your data's source of truth. Offline boot is
  enabled only after a successful prior session on that device. Logout, an
  authentication failure, or switching users clears both the in-memory and
  IndexedDB caches. The server and its `/data` volume remain the source of
  truth.

---

## 7. Desktop app (thin shell)

Alongside the browser and installable PWA, FinSight ships a small native
**desktop shell** — a single downloaded/built binary that is just a window
pointed at your server. It holds no data of its own: no local database, no
local accounts, no separate copy of your finances. It exists so you get a
real app icon, a dock/taskbar presence, and a system-tray entry, while all
state stays on your self-hosted server exactly as with any other client.

- **First launch** shows a **Connect** screen asking for your server's
  address — the same URL you'd open in a browser (a Tailscale hostname like
  `https://finsight.myhouse.ts.net`, a domain, or a LAN address). It health-
  checks the server, stores the URL in your OS keychain, and then loads the
  real app. From that point on the shell behaves exactly like the browser/PWA
  client for that server — same login, same UI, same read-only offline cache
  of last-synced data.
- **The server URL is remembered** across restarts (in the OS keychain), so
  subsequent launches skip the Connect screen and go straight to your server.
- **System tray:** left-click the tray icon to show/focus the window. The
  tray menu has **"Change Server…"** (forgets the stored URL and relaunches
  back to the Connect screen — use this to point the shell at a different
  server) and **"Quit"**.
- **Exports** (CSV/JSON) download through the webview's normal file-download
  handling — the same Blob download the browser and PWA use — so they land in
  your OS's usual downloads location, no native "save as" dialog wired
  separately.
- **No separate offline mode beyond the web client's.** The shell is the same
  web app served from your server, so its offline behavior is whatever the
  browser/PWA offers for that origin (the read-only last-synced cache); it
  does not add any additional local persistence of its own.

---

## 8. Backups & upgrades

**Per-user snapshots:** Settings → Data & backups creates encrypted snapshots
inside that user's `backups/` directory. FinSight also creates a snapshot before
applying pending database migrations and keeps the ten newest snapshots in
that user's backup directory. Restoring from the UI stages a replacement
database and takes a
pre-restore safety snapshot. Restart the server/container to apply the staged
restore before opening the account again.

**Whole-server disaster recovery:** back up the complete `/data` volume,
including `users.db` and every `users/<uuid>/` directory. Stop the service while
copying so the SQLite database, WAL, and wrapped-key registry form one
consistent snapshot:

```bash
docker compose stop finsight
docker compose cp -a finsight:/data/. ./finsight-data-backup
docker compose start finsight
```

To prove a backup before depending on it, copy `docker-compose.yml` and the
backup into an empty test directory. Put `FINSIGHT_HOST_PORT=8675` in that
directory's `.env` so it cannot collide with the live instance, then restore
into the fresh Compose project and volume:

```bash
docker compose create finsight
docker compose cp -a ./finsight-data-backup/. finsight:/data
docker compose start finsight
```

Do not copy a backup over a running instance. Restore into a fresh volume
before starting the replacement container. Keep a copy off the Docker host,
and protect it like other sensitive account data.
Although financial records and integration secrets are SQLCipher-encrypted,
`users.db` contains password verifiers and wrapped keys that can be attacked
offline; use strong passwords and encrypt or tightly restrict the backup
destination.

**Upgrades:** pull the newer image, then recreate the container —
the server takes a per-user pre-migration snapshot and then applies schema
migrations automatically when that user's runtime opens. The volume itself is
preserved:

```bash
docker compose pull
docker compose up -d
```

For reproducible deployments, set `FINSIGHT_IMAGE` in a `.env` file to a
published version tag (for example,
`ghcr.io/koushik0901/finsight:0.1.0`) instead of the moving `latest` tag.
Source-build users should pull Git changes and repeat the two-file build command
from Section 2. Already-open browser tabs (including installed PWAs) detect the
version mismatch via the server's `/api/server/about` handshake and show a
"refresh to update" banner — no manual cache-busting needed.

If you used the early single-user server preview, first-run setup also recognizes
a root-level `/data/data.sqlcipher` + `/data/db.key` pair. It moves the database
and any WAL/SHM sidecars into the new administrator's user directory as one
rollback-safe operation, then deletes the obsolete plaintext key file. An
incomplete pair is treated as a migration error instead of creating an empty
replacement database.

---

## 9. Connect Claude or ChatGPT (MCP)

FinSight speaks the **Model Context Protocol**, so you can point Claude
Desktop, claude.ai, ChatGPT, or Claude Code at your server and use your own AI
subscription instead of configuring a provider key for the in-app Copilot. The
external assistant gets the *same* tools the in-app Copilot uses — the 43-tool
set is shared code, not a reimplementation — plus five tools for reviewing and
applying proposals.

Endpoint: **`https://<your-server>/mcp`** (shown in Settings → Connections).

### Can your client reach it?

| Client | What it needs |
|---|---|
| **Claude Code**, `mcp-remote`, other local bridges | Any reachable URL — LAN, Tailscale, or `http://localhost:8674`. Uses an access token. |
| **claude.ai**, **Claude Desktop** custom connectors, **ChatGPT** | A **publicly reachable HTTPS origin**. These connect *from Anthropic's/OpenAI's servers*, not from your laptop, so `localhost` and a plain Tailscale tailnet are invisible to them. Use Recipe B (public domain + Caddy) or `tailscale funnel`. They run the OAuth flow automatically. |

Recipe A's `tailscale serve` is tailnet-only. `tailscale funnel --bg 8674`
exposes the same server publicly if you'd rather not run your own domain.

### Two ways to authenticate

**Access token (for Claude Code and local bridges).** Settings → Connections →
Create token. The token is shown **once**. Then:

```bash
claude mcp add --transport http finsight https://your-server/mcp --header "Authorization: Bearer finsight_pat_..."
```

**OAuth (for cloud connectors).** Paste `https://your-server/mcp` into the
client's "add custom connector" box. It registers itself, sends you to
FinSight's consent screen, you pick an access level, and it receives a token —
no copy-pasting. The connector then appears in Settings → Connections under its
own name, revocable like any other token.

OAuth-issued tokens are **short-lived and renew themselves**: the connector gets
an access token good for an hour plus a refresh token, and swaps them silently
in the background. You never see this happen. Revoking the connector in Settings
kills both halves, so it cannot renew its way back in. Tokens you create by hand
do *not* expire — you pasted them into a config file, and having them stop
working on a timer would just look like a broken integration.

### Widgets

Some results come back as a small card — net worth, a spending breakdown, a
transaction list, an affordability verdict — instead of a wall of JSON. This
uses the MCP Apps UI standard, so a client that supports it (ChatGPT, and Claude
surfaces as they adopt it) renders the card inline, and a client that doesn't
just shows the same numbers as text. Nothing is lost either way; the tools are
fully usable headless.

The cards are self-contained HTML with no external requests, so they work under
the strictest client CSP and never phone home.

### Access levels

- **Read only** — analysis only. The assistant can read accounts,
  transactions, budgets, goals, and every projection, but cannot change
  anything. Start here.
- **Read and write** — additionally lets the assistant *propose* changes and,
  once you agree in the conversation, apply them.

With a read-write token the flow is deliberately three steps: the assistant
drafts a proposal, tells you what it would change, and only calls approve +
execute after you say yes. Every proposal is saved as a pending bundle visible
in FinSight itself, so you can always see (and apply, or reject) what an
assistant suggested.

An assistant can only apply proposals **it** drafted. Approving is meant to
record "you agreed, in this conversation", and nothing else was there to see
that agreement — so a proposal made inside FinSight, or by a different
connected assistant, comes back as a refusal rather than being applied. Those
stay yours to review in the app. (If you have both Claude and ChatGPT connected
and ask the second one to apply the first one's suggestion, this is why it
declines.)

### Security notes

- A token is stored only as a hash, and it wraps its own copy of your database
  key — so a stolen `users.db` yields neither a usable token nor readable data.
  The flip side: **the token itself unlocks your financial data**, so treat it
  like a password.
- Resetting your password with a recovery key **revokes every token**,
  including OAuth-issued connectors. Reconnect afterwards.
- `/mcp` never accepts session cookies, only bearer tokens.
- If discovery reports the wrong origin behind your proxy (an `http://` issuer,
  or an internal hostname), set **`FINSIGHT_PUBLIC_ORIGIN=https://your-server`**
  in the container environment.
- Tokens travel in an `Authorization` header. Reverse proxies don't log request
  headers by default; if you've enabled that, exclude it.

---

## 10. Current limits

- **Long-lived Copilot streaming requests.** Chat answers stream over a
  single held-open HTTP request. Some reverse proxies cut idle connections
  after 30–60 seconds by default. Caddy's default timeouts are generous
  enough for this out of the box; if you're using nginx or another proxy and
  see Copilot answers truncate mid-stream, raise its read/proxy timeout
  (e.g. nginx's `proxy_read_timeout`) well above your typical answer length.
- **No CSV share-target yet.** On Android, sharing a downloaded CSV directly
  into the installed PWA (via the OS share sheet) isn't wired up yet — import
  CSVs through the in-app import flow instead. (iOS Safari doesn't support
  share targets for PWAs at all, so this is an Android-only gap regardless.)
- **Offline is read-only.** The offline cache lets you *view* last-synced
  balances, budgets, and transactions with no connection. It does not queue
  edits made while offline — mutations (adding a transaction, editing a
  budget, etc.) require connectivity and are paused, not queued, while
  you're offline.
