# Self-hosting FinSight

FinSight's server mode (`finsight-server`) is an Immich-style self-hosted
service: it runs on hardware you control, stores every account's data in a
per-user encrypted SQLCipher database under a single data directory, and
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
- **All state lives under one directory, mounted as `/data` inside the
  container.** That includes `users.db` (the account/session registry) and
  one encrypted SQLCipher database per user. Back this directory up; there is
  nothing else to back up.
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

The first run builds the image from the included `Dockerfile` (a few minutes
— it compiles the Rust server release binary and the UI), then starts the
container in the background, publishing port `8674` and creating a named
volume (`finsight-data`) for `/data`.

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

---

## 3. Recipe A — Tailscale (recommended)

The easiest path if you don't want to manage a domain, certificates, or
port-forwarding at all. [Tailscale](https://tailscale.com/) is a mesh VPN;
its `serve` feature also gets you a real, browser-trusted HTTPS certificate
for free via Let's Encrypt, scoped to your own private tailnet — no public
DNS record, no open port on your router.

1. Install Tailscale on the host running FinSight, and on every device
   you'll use to access it (phone, laptop, etc.), then sign them into the
   same tailnet: <https://tailscale.com/download>.
2. Enable [MagicDNS](https://tailscale.com/kb/1081/magicdns) and HTTPS
   certificates for your tailnet in the Tailscale admin console (Settings →
   enable "HTTPS Certificates").
3. On the FinSight host, point Tailscale's built-in reverse proxy at the
   container:

   ```bash
   tailscale serve https / http://localhost:8674
   ```

   This terminates TLS with a certificate Tailscale manages and issues for
   you, and forwards to the FinSight container over plain HTTP on the
   loopback interface — Docker's own port publish (`8674:8674`) is what
   makes `localhost:8674` reachable here.

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

3. Add a `caddy` service to `docker-compose.yml` on the same Docker network
   as `finsight`, and drop the host port mapping on `finsight` itself (Caddy
   is now the only thing facing outward):

   ```yaml
   services:
     finsight:
       build: .
       restart: unless-stopped
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
  you're offline — never as your data's source of truth. The server, and
  its `/data` volume, is always the source of truth; the offline cache is
  just a read-through window into what it last saw.

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

**Backups:** everything that matters is the `/data` volume (`users.db` +
each user's encrypted database). Snapshot it however suits your setup:

```bash
# Named-volume backup to a local tarball
docker run --rm -v finsight-data:/data -v "$(pwd)":/backup debian:bookworm-slim \
  tar czf /backup/finsight-backup-$(date +%Y%m%d).tar.gz -C /data .
```

Restore by extracting that tarball back into a fresh `finsight-data` volume
before starting the container. Keep backups off the host itself (another
disk, another machine, a small offsite/cloud copy of the encrypted archive)
— the whole point of the per-user encryption is that a copy of the archive
alone isn't useful without the corresponding password/recovery key, so it's
safe to store it somewhere less trusted than the live server.

**Upgrades:** pull or rebuild a newer image, then recreate the container —
the database schema migrates automatically on startup, and the volume is
untouched:

```bash
git pull                 # if building locally from this repo
docker compose build
docker compose up -d
```

Or, if you publish/consume a tagged image instead of building locally,
update the `image:` line in `docker-compose.yml` and `docker compose pull &&
docker compose up -d`. Already-open browser tabs (including installed PWAs)
detect the version mismatch via the server's `/api/server/about` handshake
and show a "refresh to update" banner — no manual cache-busting needed.

---

## 9. Known limits (Phase 3)

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
