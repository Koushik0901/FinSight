# Troubleshooting

## Login

| Symptom | Likely cause | Fix |
|---|---|---|
| Cookie not set after login | `FINSIGHT_COOKIE_SECURE=1` while on bare HTTP | Set `0` for LAN tests; `1` behind HTTPS |
| 401 on every RPC | Session expired or revoked | Sign in again; check `docker compose logs` for throttling |
| Throttled for 60s | 5 failures on a username | Wait; the same throttle applies to unknown usernames |
| Stuck on setup wizard | First account already exists | Navigate to `/api/openapi.json` — if it returns, the server is up and you are on the wrong `/data` |

## Server

| Symptom | Check |
|---|---|
| Container restarts | `docker compose logs -f finsight`, `RUST_LOG=debug` |
| Port 8674 busy | Change `FINSIGHT_HOST_PORT` in `.env` |
| Wrong data after upgrade | Did you back up the whole `/data` volume, not just one per-user file? |
| Proxy shows plain HTTP | Set `FINSIGHT_PUBLIC_ORIGIN=https://your.domain` when headers are insufficient |

## Sync

| Symptom | Check |
|---|---|
| SimpleFIN sync fails | Verify access URL at the bridge; check sync run error in Accounts |
| Nothing imported | Some banks delay availability; try again in an hour |
| Duplicate transactions | Re-import dedup window; confirm transfer linking |

## Copilot

| Symptom | Check |
|---|---|
| No streaming / hangs | Provider key valid? Base URL reachable? `FINSIGHT_PUBLIC_ORIGIN` matches HTTPS origin? |
| Totally generic answer | Question may lack ledger scope — add “last month” or an account |
| Unconverted currency warning | `CurrencyContext` listed — disambiguate currency or add a rate |

## CSV import

| Symptom | Fix |
|---|---|
| Wrong sign | Flip sign mapping in the import preview |
| Columns misaligned | Some banks change column order — re-map in the drawer |
| File not accepted | Check the supported parser list in `finsight-providers`; file may be in an unsupported bank variant |

## PWA

| Symptom | Fix |
|---|---|
| Not installable | Needs HTTPS (or `localhost`); bare `http://<lan-ip>` cannot install |
| Offline blank | Cache only writes in a secure context — use HTTPS |
| Push / share-target inert | Depends on service worker; see above |

Logs are the fastest path: `docker compose logs -f finsight`.
