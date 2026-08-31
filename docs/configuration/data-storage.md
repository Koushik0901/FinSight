# Data Storage

Where FinSight keeps data, and what to back up.

## On-disk layout

```text
/data                  # Docker volume finsight-data; native dev ./data
├── users.db            # account registry — usernames, Argon2id verifiers, wrapped DB keys, hashed sessions
├── session.key         # wraps persisted session keys — back this up
└── users/<user-uuid>/
    ├── data.sqlcipher   # per-user ledger, budgets, goals, secrets (LLM keys, SimpleFIN URLs)
    ├── backups/         # manual + pre-migration snapshots
    └── imports/         # authenticated CSV staging
```

- `users.db` is **plain SQLite**, not SQLCipher — it must be readable before any user key is unwrapped. It contains no financial records, plaintext passwords, or plaintext database keys.
- `data.sqlcipher` is **SQLCipher-encrypted** with a random 32-byte key generated per user.
- Databases are migrated via Refinery files in `crates/finsight-core/migrations` (currently through `V016`, next is `V017__`).

## Key wrapping

Per `crypto.rs`:

- The DB key is stored only wrapped, twice:
  - `Wrap(KEK1)` where `KEK1 = Argon2id(password, kek_salt)` pinned at m=19456 KiB, t=2, p=1, id v19
  - `Wrap(KEK2)` where `KEK2 = recovery key bytes` (high-entropy, no KDF)
- Password verification uses a **separate** Argon2id PHC string (own salt) so the verifier cannot derive the KEK.
- Unwrapped keys exist only in server memory, single-flighted, evicted after 30 minutes idle (unless SSE attached).

## Browser cache

A seven-day, **read-only, IndexedDB query cache** (tanstack-query persist) for offline viewing. Encrypted when the PWA has a secure context; purged on logout or authentication failure. The vault is `/data`, not the cache.

## Backups

- **Per-user snapshots** — Settings → Data & backups creates/restores an encrypted snapshot under `users/<uuid>/backups/` on the server.
- **Disaster recovery** — back up the **whole `/data` volume** so `users.db` + `session.key` + every per-user database stay together. A single per-user file without `users.db` cannot be unwrapped.

## SimpleFIN & provider secrets

LLM API keys and SimpleFIN access URLs are rows in `data.sqlcipher`, per user — not a shared keychain. Deleting a user removes that user’s database and all secrets inside it.

See also: [Privacy & Local Data](/getting-started/privacy), [Security & Privacy](/help/security).
