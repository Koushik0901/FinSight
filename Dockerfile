# ---- 1. Build the UI ----
# FinSight's UI lives in a pnpm workspace (root pnpm-lock.yaml + pnpm-workspace.yaml
# listing "ui"); there is no standalone ui/package-lock.json, so this stage uses
# pnpm (via corepack) rather than `npm ci`.
FROM node:20-bookworm-slim AS ui
WORKDIR /repo
RUN corepack enable && corepack prepare pnpm@9.0.0 --activate
COPY package.json pnpm-lock.yaml pnpm-workspace.yaml ./
COPY ui/package.json ui/package.json
RUN pnpm install --frozen-lockfile
COPY ui/ ui/
RUN pnpm --filter ui build     # → /repo/ui/dist (PWA manifest + sw + assets)

# ---- 2. Build the server (release) ----
FROM rust:1-bookworm AS server
# perl/make: vendored OpenSSL/SQLCipher build. pkg-config + libdbus-1-dev: the
# workspace's `keyring` dependency (finsight-core's keychain module) pulls in
# the `dbus`/libdbus-sys secret-service backend on Linux even though
# finsight-server never calls it — cargo still needs to link the crate.
RUN apt-get update && apt-get install -y --no-install-recommends perl make pkg-config libdbus-1-dev && rm -rf /var/lib/apt/lists/*
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY src-tauri/ src-tauri/
# Build ONLY the server bin — never the Tauri app (no webkit deps in this image).
RUN cargo build --release -p finsight-server

# ---- 3. Slim runtime ----
FROM debian:bookworm-slim AS runtime
# libdbus-1-3: runtime counterpart of the builder's libdbus-1-dev (see note above).
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates libdbus-1-3 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=server /src/target/release/finsight-server /usr/local/bin/finsight-server
COPY --from=ui /repo/ui/dist /app/ui/dist
ENV FINSIGHT_DATA_DIR=/data \
    FINSIGHT_UI_DIR=/app/ui/dist \
    FINSIGHT_PORT=8674 \
    FINSIGHT_COOKIE_SECURE=1 \
    RUST_LOG=info
VOLUME /data
EXPOSE 8674
# /dev/tcp is a bash extension, not POSIX — /bin/sh on bookworm-slim is dash,
# which would make this healthcheck fail forever. bash ships as a required
# Debian package (present without an explicit install), just not wired as sh.
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s CMD ["/bin/bash","-c","exec 3<>/dev/tcp/127.0.0.1/8674 && printf 'GET /api/health HTTP/1.0\\r\\n\\r\\n' >&3 && grep -q '\"status\":\"ok\"' <&3"]
ENTRYPOINT ["/usr/local/bin/finsight-server"]
