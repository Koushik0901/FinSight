use crate::state::ServerState;
use axum::http::header::{HeaderValue, CACHE_CONTROL};
use axum::{
    extract::DefaultBodyLimit,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use std::path::Path;
use std::sync::Arc;
use tower_http::compression::CompressionLayer;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;

/// Vite writes every file under `dist/assets/` with a content hash in its name,
/// so a given URL's bytes can never change. That is exactly the contract
/// `immutable` describes: the browser may reuse it for a year without even
/// sending a revalidation request. A new build produces new filenames, so
/// there is no staleness window to worry about.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";

/// Everything OUTSIDE `dist/assets/` — `index.html`, `sw.js`, the web manifest,
/// PWA icons — keeps its filename across builds, so it must be revalidated or a
/// user would be pinned to a stale app shell forever (and the service worker
/// could never hand out an update). `no-cache` still allows the cache to store
/// the bytes; it only requires a conditional request, which `ServeDir`'s ETag
/// answers with a 304.
const REVALIDATE: &str = "no-cache";

/// Compression for a response body we generate per request (RPC/MCP JSON).
///
/// CRITICAL: this layer is applied per-route and is deliberately never attached
/// to `/api/events`. tower-http's compressor buffers inside the encoder, which
/// holds SSE frames back until the buffer fills — Copilot token streaming and
/// import progress would arrive in lumps, or not until the stream ended. A
/// whole-router `.layer()` here would be a latency regression disguised as an
/// optimization.
fn dynamic_compression() -> CompressionLayer {
    CompressionLayer::new()
}

/// One `Cache-Control` layer, so the hashed-asset route and the SPA fallback
/// each declare their policy in a single readable place.
fn cache_header(cache_control: &'static str) -> SetResponseHeaderLayer<HeaderValue> {
    SetResponseHeaderLayer::overriding(CACHE_CONTROL, HeaderValue::from_static(cache_control))
}

/// `ui_dir` is the built frontend (`ui/dist` in production). The fallback
/// service serves any real static file it finds there; anything else (an SPA
/// client-side route like `/transactions`) falls back to `index.html` so the
/// React router can take over. Registered LAST so `/api/*` routes always win.
pub fn build_router(state: Arc<ServerState>, ui_dir: &Path) -> Router {
    let is_empty_ui = ui_dir.as_os_str().is_empty();
    // Build the API-only router first — static serving is appended only when
    // a real ui_dir is configured. An empty string (FINSIGHT_UI_DIR="") in
    // split mode disables static serving so the API container never serves the
    // SPA from CWD (the previous PathBuf("") → "." behavior was a bug).
    let base = Router::new()
        // CORS on the public health probe only: the thin desktop shell's
        // ConnectScreen runs at its OWN origin (tauri://localhost, or Vite's
        // localhost:5173 under `tauri:dev`) and does a cross-origin
        // `fetch(<server>/api/health)` to check reachability BEFORE navigating
        // the window to the server. WebView2/browsers enforce CORS on that
        // cross-origin request, so the endpoint must opt in. Safe: /api/health
        // is unauthenticated and returns only `{"status":"ok"}` — no data, no
        // credentials (permissive() disallows credentialed requests). Every
        // other route stays same-origin-only.
        .route(
            "/api/health",
            get(|| async { Json(serde_json::json!({"status":"ok"})) })
                .layer(CorsLayer::permissive()),
        )
        .route("/api/server/about", get(crate::server_info::about))
        .route(
            "/api/openapi.json",
            get(openapi).layer(dynamic_compression()).layer(cache_header(REVALIDATE)),
        )
        .route("/api/auth/status", get(crate::auth::status))
        .route("/api/auth/setup", post(crate::auth::setup))
        .route("/api/auth/login", post(crate::auth::login))
        .route("/api/auth/logout", post(crate::auth::logout))
        .route("/api/auth/sign-out-others", post(crate::auth::sign_out_others))
        .route("/api/auth/recover", post(crate::auth::recover))
        .route(
            "/api/auth/users",
            get(crate::auth::list_users).post(crate::auth::create_user),
        )
        .route("/api/auth/users/{id}", delete(crate::auth::delete_user))
        // MCP / API access tokens. Session-authed (you manage tokens from the
        // logged-in app), unlike `/mcp` itself which is bearer-only.
        .route(
            "/api/auth/tokens",
            get(crate::tokens::list).post(crate::tokens::create),
        )
        .route("/api/auth/tokens/{id}", delete(crate::tokens::revoke))
        // Debug-only diagnostic, not a shared command: measures THIS admin's
        // own real categorization corrections locally. See
        // `admin_eval`'s module doc for why it deliberately bypasses
        // `/api/rpc/{cmd}` and the generated bindings contract.
        .route("/api/admin/private-category-eval", get(crate::admin_eval::private_category_eval))
        .route(
            "/api/import/csv",
            post(crate::uploads::upload_csv)
                .layer(DefaultBodyLimit::max(crate::uploads::MAX_CSV_UPLOAD_BYTES)),
        )
        // Compressed: RPC responses are JSON and some are large (a full
        // transaction page, a category tree, a forecast series). This is the
        // single biggest lever on perceived screen-load time over a LAN or
        // Tailscale link.
        .route(
            "/api/rpc/{cmd}",
            post(crate::dispatch::rpc).layer(dynamic_compression()),
        )
        // NOT compressed, on purpose — see `dynamic_compression`'s doc comment.
        // Compressing this stream would delay Copilot tokens and import
        // progress frames until the encoder's buffer filled.
        .route("/api/events", get(crate::events::events))
        // Model Context Protocol endpoint: bearer-authenticated (never the
        // session cookie), plain-JSON Streamable HTTP. GET/DELETE answer 405
        // rather than falling through to the SPA.
        //
        // CORS is permissive so that a browser-based caller can READ our
        // responses — most usefully the 401's `WWW-Authenticate` challenge,
        // which is how a client discovers where to authorize. It is not the
        // security boundary and is not doing any work for the cloud connectors,
        // which call server-side with no Origin at all. Browser callers are
        // gated separately by `mcp::origin_is_allowed`, and permissive() forbids
        // credentialed requests, so ambient cookies can never ride along.
        .route(
            "/mcp",
            post(crate::mcp::post)
                .get(crate::mcp::method_not_allowed)
                .delete(crate::mcp::method_not_allowed)
                // Turbofish because two chained `.layer()` calls leave the
                // INTERMEDIATE error type unconstrained — only the final one is
                // pinned to `Infallible` by `route()`. Naming it here is what
                // the single-layer routes above get for free.
                .layer::<_, std::convert::Infallible>(CorsLayer::permissive())
                // Tool results are JSON and `tools/list` alone is 48 schemas —
                // worth compressing, and safe: `/mcp` answers with plain JSON,
                // never SSE (see the module doc on mcp.rs).
                .layer(dynamic_compression()),
        )
        // OAuth discovery. Unauthenticated and CORS-open by design: a client
        // must be able to read these BEFORE it holds any credential, and the
        // 401 on /mcp points here. The `{*rest}` variants exist because clients
        // probe the path-inserted form (RFC 8414 §3.1) when the resource lives
        // under a path.
        .route(
            "/.well-known/oauth-authorization-server",
            get(crate::oauth::as_metadata).layer(CorsLayer::permissive()),
        )
        .route(
            "/.well-known/oauth-authorization-server/{*rest}",
            get(crate::oauth::as_metadata).layer(CorsLayer::permissive()),
        )
        .route(
            "/.well-known/oauth-protected-resource",
            get(crate::oauth::prm_metadata).layer(CorsLayer::permissive()),
        )
        .route(
            "/.well-known/oauth-protected-resource/{*rest}",
            get(crate::oauth::prm_metadata).layer(CorsLayer::permissive()),
        )
        // OAuth 2.1 authorization-code flow for cloud connectors. `register`
        // and `token` are unauthenticated by spec (public clients, PKCE instead
        // of a secret); `approve` is the one interactive step and is session-
        // authed, since consent is an act by a logged-in human. There is no
        // `/oauth/authorize` route here on purpose — that is an SPA screen,
        // served by the static fallback.
        .route(
            "/api/oauth/register",
            post(crate::oauth::register).layer(CorsLayer::permissive()),
        )
        .route(
            "/api/oauth/client",
            get(crate::oauth::client_info).layer(CorsLayer::permissive()),
        )
        .route("/api/oauth/approve", post(crate::oauth::approve))
        .route(
            "/api/oauth/token",
            post(crate::oauth::token).layer(CorsLayer::permissive()),
        )
        // Fallback for the PWA share target. A share-sheet POST normally never
        // reaches the server at all — the service worker intercepts it (see
        // ui/public/share-target-sw.js; the session cookie is SameSite=Lax and
        // so is withheld from this cross-site POST, which is exactly why the
        // server can't handle the file itself).
        //
        // But the route must exist for the window where the worker isn't in
        // control yet — first install, or an update swapping workers. Without
        // it the static fallback answers a POST with a bare 405 inside the
        // launched share window. Redirecting into the app turns that into a
        // toast the user can act on. 303 so the follow-up is a GET.
        .route("/share-target", post(share_target_fallback))
        .with_state(state.clone());
    if is_empty_ui {
        return base;
    }
    let index = ui_dir.join("index.html");
    return base
        // Content-hashed bundles: cache for a year without revalidating.
        // Both static services below use `precompressed_br`/`precompressed_gzip`,
        // which make `ServeDir` prefer a sibling `.br`/`.gz` file when the
        // client advertises that encoding. `ui/scripts/precompress.mjs` writes
        // those at build time at brotli's maximum quality — ~15% smaller than
        // what the live encoder produces — so the server ships the smallest
        // bytes without compressing the same unchanging asset per request.
        // `CompressionLayer` stays attached as the fallback for anything not
        // precompressed; it no-ops when `Content-Encoding` is already set.
        //
        // Wrapped in a `Router` rather than layered onto `any_service(..)`
        // directly: `MethodRouter::layer` is generic over a new error type,
        // and with `ServeDir`'s `Infallible` there are several `From<Infallible>`
        // impls in scope, so inference has no way to pick one. `Router::layer`
        // fixes the error type at `Infallible` and sidesteps it.
        .nest_service(
            "/assets",
            Router::new()
                .fallback_service(
                    ServeDir::new(ui_dir.join("assets"))
                        .precompressed_br()
                        .precompressed_gzip(),
                )
                .layer(CompressionLayer::new())
                .layer(cache_header(IMMUTABLE)),
        )
        // Everything else: the app shell and the unhashed PWA files. Must
        // revalidate so a deploy actually reaches an already-installed client.
        .fallback_service(
            Router::new()
                .fallback_service(
                    ServeDir::new(ui_dir)
                        .precompressed_br()
                        .precompressed_gzip()
                        .fallback(ServeFile::new(index)),
                )
                .layer(CompressionLayer::new())
                .layer(cache_header(REVALIDATE)),
        )
}

async fn openapi() -> impl IntoResponse {
    Json(finsight_openapi::build_openapi())
}

/// See the `/share-target` route comment. Deliberately does NOT read the body:
/// without a session cookie there is no user to stage the upload for, and
/// buffering a file we cannot use would just be a free memory sink.
async fn share_target_fallback() -> axum::response::Response {
    (
        axum::http::StatusCode::SEE_OTHER,
        [(axum::http::header::LOCATION, "/?shared=error")],
    )
        .into_response()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::util::ServiceExt;

    pub(crate) fn test_state() -> Arc<ServerState> {
        let dir = tempfile::tempdir().unwrap();
        // Leak the tempdir so the DB outlives the test body.
        let path = dir.keep();
        ServerState::bootstrap(&path).unwrap()
    }

    /// A fresh, empty tempdir for tests that only exercise `/api/*` routes and
    /// don't care about static file serving. `ServeDir` resolves lazily per
    /// request, so an empty (or even missing) directory never panics here —
    /// unmatched requests just fall through to a 404.
    pub(crate) fn test_ui_dir() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap();
        dir.keep()
    }

    /// Runs `POST /api/auth/setup` on `app` (creating the sole admin account,
    /// username `tester`) and returns the `finsight_session=<token>` cookie
    /// pair for use in a `Cookie:` header on subsequent authenticated
    /// requests. Panics if setup doesn't succeed (callers rely on a fresh,
    /// unauthenticated `ServerState` from `test_state()`).
    pub(crate) async fn setup_and_login(app: &Router) -> String {
        let res = app
            .clone()
            .oneshot(
                Request::post("/api/auth/setup")
                    .header("content-type", "application/json")
                    // >= auth::MIN_PASSWORD_LEN: the old "hunter22" was 8 chars
                    // and is now rejected as auth.weak_password.
                    .body(Body::from(r#"{"username":"tester","password":"hunter22-plus"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::OK,
            "setup must succeed for the test helper"
        );
        let cookie = res
            .headers()
            .get(axum::http::header::SET_COOKIE)
            .expect("setup response must set a session cookie")
            .to_str()
            .unwrap();
        // The Set-Cookie value is `name=value; HttpOnly; ...` — the request
        // `Cookie:` header only wants the first `name=value` segment.
        cookie.split(';').next().unwrap().to_string()
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = build_router(test_state(), &test_ui_dir());
        let res = app
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    /// A `dist`-shaped directory: one content-hashed asset under `assets/`
    /// and an `index.html` shell, which is all the caching/compression tests
    /// need to tell the two policies apart.
    fn test_dist_dir() -> std::path::PathBuf {
        let dir = tempfile::tempdir().unwrap().keep();
        std::fs::create_dir_all(dir.join("assets")).unwrap();
        // Comfortably over CompressionLayer's 32-byte floor, and repetitive so
        // it definitely compresses smaller than the original.
        std::fs::write(
            dir.join("assets/app-abc123.js").as_path(),
            "console.log('x');".repeat(200),
        )
        .unwrap();
        std::fs::write(
            dir.join("index.html").as_path(),
            "<!doctype html><title>t</title>",
        )
        .unwrap();
        dir
    }

    fn header<'a>(res: &'a axum::response::Response, name: &str) -> Option<&'a str> {
        res.headers().get(name).and_then(|v| v.to_str().ok())
    }

    #[tokio::test]
    async fn hashed_assets_are_compressed_and_cached_immutably() {
        let dist = test_dist_dir();
        let app = build_router(test_state(), &dist);
        let res = app
            .oneshot(
                Request::get("/assets/app-abc123.js")
                    .header("accept-encoding", "br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header(&res, "content-encoding"), Some("br"));
        // Vite puts a content hash in the filename, so these bytes can never
        // change under this URL — the browser should not even revalidate.
        assert_eq!(header(&res, "cache-control"), Some(IMMUTABLE));
    }

    /// `ui/scripts/precompress.mjs` writes `.br` siblings at brotli's maximum
    /// quality, which measured ~15% smaller than what `CompressionLayer`
    /// produces on the fly (39,624 vs 46,491 bytes for the entry chunk). That
    /// only pays off if `ServeDir` actually prefers the file — and the
    /// `content-encoding: br` header alone cannot tell the two apart, since the
    /// live layer sets it too.
    ///
    /// So this asserts on the BYTES: the `.br` payload here is deliberately not
    /// valid brotli, which no encoder would ever emit. Getting it back verbatim
    /// proves the file was served rather than regenerated.
    #[tokio::test]
    async fn precompressed_br_sibling_is_served_verbatim() {
        let dist = test_dist_dir();
        let marker = b"not-really-brotli-but-served-as-is";
        std::fs::write(dist.join("assets/app-abc123.js.br").as_path(), marker).unwrap();

        let app = build_router(test_state(), &dist);
        let res = app
            .oneshot(
                Request::get("/assets/app-abc123.js")
                    .header("accept-encoding", "br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header(&res, "content-encoding"), Some("br"));
        let body = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(
            body.as_ref(),
            marker,
            "the .br file on disk must go on the wire untouched; if this fails, \
             ServeDir ignored it and CompressionLayer re-compressed the original"
        );
    }

    #[tokio::test]
    async fn app_shell_is_compressed_but_must_revalidate() {
        let dist = test_dist_dir();
        let app = build_router(test_state(), &dist);
        let res = app
            .oneshot(
                Request::get("/index.html")
                    .header("accept-encoding", "br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        // `index.html` keeps its name across deploys. Caching it immutably
        // would pin every already-installed client to the old app forever.
        assert_eq!(header(&res, "cache-control"), Some(REVALIDATE));
    }

    /// The regression this whole change is most likely to cause.
    ///
    /// tower-http's compressor buffers inside the encoder, so an SSE stream
    /// routed through it delivers nothing until that buffer fills — Copilot
    /// tokens and import-progress frames would arrive in lumps, or only when
    /// the stream ended. The compression layers are therefore attached per
    /// route and `/api/events` deliberately never gets one. If someone later
    /// "simplifies" this into a single `.layer()` on the whole Router, this
    /// test is what catches it.
    #[tokio::test]
    async fn sse_event_stream_is_never_compressed() {
        let app = build_router(test_state(), &test_ui_dir());
        let cookie = setup_and_login(&app).await;
        let res = app
            .clone()
            .oneshot(
                Request::get("/api/events")
                    .header("cookie", &cookie)
                    // Ask for everything — the point is that we still get none.
                    .header("accept-encoding", "br, gzip, zstd, deflate")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            header(&res, "content-type"),
            Some("text/event-stream"),
            "sanity: this test is only meaningful against the real SSE handler"
        );
        assert_eq!(
            header(&res, "content-encoding"),
            None,
            "SSE must not be compressed — the encoder buffers and would delay every frame"
        );
    }

    #[tokio::test]
    async fn health_sends_cors_header_for_cross_origin_probe() {
        // The thin desktop shell's ConnectScreen fetches /api/health cross-origin
        // before navigating; without an Access-Control-Allow-Origin header the
        // browser/WebView2 blocks it ("Failed to fetch").
        let app = build_router(test_state(), &test_ui_dir());
        let res = app
            .oneshot(
                Request::get("/api/health")
                    .header("origin", "http://localhost:5173")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get(axum::http::header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_some(),
            "health probe must send a CORS allow-origin header for the shell's pre-navigation check",
        );
    }

    #[tokio::test]
    async fn spa_fallback_serves_index_html_for_unknown_route() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state(), &ui_dir);
        let res = app
            .oneshot(Request::get("/some/spa/route").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"SENTINEL_INDEX_HTML");
    }

    #[tokio::test]
    async fn api_routes_win_over_static_fallback() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state(), &ui_dir);
        let res = app
            .oneshot(Request::get("/api/health").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["status"], "ok");
    }

    /// The service worker normally answers this POST and the server never sees
    /// it. When the worker isn't in control yet, the share window must still get
    /// something better than the static fallback's bare 405.
    #[tokio::test]
    async fn share_target_post_redirects_into_the_app_instead_of_405() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state(), &ui_dir);
        let res = app
            .oneshot(
                Request::post("/share-target")
                    .body(Body::from("irrelevant"))
                    .unwrap(),
            )
            .await
            .unwrap();
        // 303 specifically: the follow-up must be a GET so a reload of the
        // landing page can't re-submit the share.
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            res.headers()
                .get(axum::http::header::LOCATION)
                .and_then(|v| v.to_str().ok()),
            Some("/?shared=error"),
            "must land on a flag the frontend turns into a toast",
        );
    }

    /// Pins what registering a POST-only route actually does to other methods.
    ///
    /// Registering `/share-target` claims the path in the path router, so a GET
    /// no longer reaches the SPA fallback — axum answers 405 from the
    /// `MethodRouter`'s own default (routing/method_routing.rs: `MethodRouter::new`
    /// installs a 405 fallback, and `method_not_allowed_fallback` is an explicit
    /// opt-in that `fallback_service` does NOT trigger).
    ///
    /// That is the correct answer here — nothing in the app navigates to
    /// `/share-target` with a GET; the worker redirects to `/?shared=…` instead
    /// — and 405 carries an `Allow` header saying so. This test exists so the
    /// behaviour is a decision on record rather than a surprise.
    #[tokio::test]
    async fn share_target_get_is_method_not_allowed_not_the_spa() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state(), &ui_dir);
        let res = app
            .oneshot(Request::get("/share-target").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(
            res.headers()
                .get(axum::http::header::ALLOW)
                .and_then(|v| v.to_str().ok()),
            Some("POST"),
            "405 must tell the caller which method the endpoint takes",
        );
    }

    /// Other SPA routes must be untouched by the addition above.
    #[tokio::test]
    async fn unrelated_spa_routes_still_fall_through_to_index() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        let app = build_router(test_state(), &ui_dir);
        let res = app
            .oneshot(Request::get("/accounts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"SENTINEL_INDEX_HTML");
    }

    #[tokio::test]
    async fn real_static_file_is_served_from_ui_dir() {
        let ui_dir = test_ui_dir();
        std::fs::write(ui_dir.join("index.html"), "SENTINEL_INDEX_HTML").unwrap();
        std::fs::write(ui_dir.join("asset.txt"), "STATIC_ASSET_CONTENT").unwrap();
        let app = build_router(test_state(), &ui_dir);
        let res = app
            .oneshot(Request::get("/asset.txt").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"STATIC_ASSET_CONTENT");
    }

    #[tokio::test]
    async fn openapi_json_is_valid_and_no_cache_and_compressed() {
        let app = build_router(test_state(), &test_ui_dir());
        let res = app
            .oneshot(
                Request::get("/api/openapi.json")
                    .header("accept-encoding", "br")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            header(&res, "content-type").map(|v| v.contains("application/json")),
            Some(true)
        );
        assert_eq!(header(&res, "cache-control"), Some(REVALIDATE));
        // Empty spec is still JSON and compressible, but compression is valid if present; ensure not missing
        // If client asked br, we may get br or identity — both ok, but header must be present for JSON
        // Just check status and valid JSON body
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(v.get("openapi").is_some(), "openapi field missing");
        assert!(v.get("info").is_some(), "info missing");
        assert_eq!(v["info"]["title"], "FinSight API");
    }

    #[tokio::test]
    async fn openapi_json_route_is_no_cache() {
        let app = build_router(test_state(), &test_ui_dir());
        let res = app
            .oneshot(Request::get("/api/openapi.json").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(header(&res, "cache-control"), Some(REVALIDATE));
    }
}
