//! MCP endpoint integration tests: bearer auth, JSON-RPC framing, the tool
//! catalogue, scope enforcement, and the full draft → approve → execute loop an
//! external assistant drives. Runs the real router over
//! `tower::ServiceExt::oneshot`, with tokens minted through the real REST
//! endpoint so the whole credential chain is exercised.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use finsight_server::router::build_router;
use finsight_server::state::ServerState;
use std::path::PathBuf;
use std::sync::Arc;
use tower::util::ServiceExt;

type App = axum::Router;

fn fresh_state() -> (Arc<ServerState>, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.keep();
    let state = ServerState::bootstrap(&path).unwrap();
    (state, path)
}

fn test_ui_dir() -> PathBuf {
    tempfile::tempdir().unwrap().keep()
}

async fn json_body(res: axum::response::Response) -> serde_json::Value {
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

fn cookie_from(res: &axum::response::Response) -> String {
    res.headers()
        .get(header::SET_COOKIE)
        .expect("expected a session cookie")
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_string()
}

/// Sets up an account and returns `(app, session cookie, full-scope token)`.
async fn setup_with_token(scope: &str) -> (App, String, String) {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/setup")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"alice","password":"hunter22-plus"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = cookie_from(&res);
    let token = mint_token(&app, &cookie, scope).await;
    (app, cookie, token)
}

async fn mint_token(app: &App, cookie: &str, scope: &str) -> String {
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/tokens")
                .header("content-type", "application/json")
                .header("cookie", cookie)
                .body(Body::from(
                    serde_json::json!({"name":"Claude Desktop","scope":scope}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    json_body(res).await["token"].as_str().unwrap().to_string()
}

fn mcp_req(token: &str, body: serde_json::Value) -> Request<Body> {
    Request::post("/mcp")
        .header("content-type", "application/json")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn rpc(id: i64, method: &str, params: serde_json::Value) -> serde_json::Value {
    serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

/// One `tools/call`, returning the JSON-RPC response body.
async fn call_tool(
    app: &App,
    token: &str,
    name: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    let res = app
        .clone()
        .oneshot(mcp_req(
            token,
            rpc(9, "tools/call", serde_json::json!({"name": name, "arguments": args})),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "tools/call {name} should be HTTP 200");
    json_body(res).await
}

fn rpc_req(cmd: &str, cookie: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(format!("/api/rpc/{cmd}"))
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(body.to_string()))
        .unwrap()
}

// ------------------------------------------------------------ handshake ---

#[tokio::test]
async fn initialize_advertises_tools_and_usage_instructions() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let res = app
        .clone()
        .oneshot(mcp_req(
            &token,
            rpc(
                1,
                "initialize",
                serde_json::json!({
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": {"name": "claude-desktop", "version": "1.0"}
                }),
            ),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    assert_eq!(body["jsonrpc"], "2.0");
    assert_eq!(body["id"], 1);
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    assert!(body["result"]["capabilities"]["tools"].is_object());
    assert_eq!(body["result"]["serverInfo"]["name"], "finsight");
    let instructions = body["result"]["instructions"].as_str().unwrap();
    assert!(instructions.contains("_display"));
    assert!(instructions.contains("explicitly agree"));
}

#[tokio::test]
async fn protocol_version_is_negotiated_not_blindly_echoed() {
    let (app, _cookie, token) = setup_with_token("full").await;

    // Every version we speak comes back unchanged. Older clients are still out
    // there, so keeping the older revisions working matters as much as adding
    // the newest one.
    for spoken in ["2025-03-26", "2025-06-18", "2025-11-25"] {
        let body = json_body(
            app.clone()
                .oneshot(mcp_req(
                    &token,
                    rpc(1, "initialize", serde_json::json!({"protocolVersion": spoken})),
                ))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(body["result"]["protocolVersion"], spoken);
    }

    // ...one we don't falls back to our default rather than agreeing to
    // something we can't honour.
    let body = json_body(
        app.oneshot(mcp_req(
            &token,
            rpc(2, "initialize", serde_json::json!({"protocolVersion": "1999-01-01"})),
        ))
        .await
        .unwrap(),
    )
    .await;
    assert_eq!(body["result"]["protocolVersion"], "2025-11-25");
}

#[tokio::test]
async fn notifications_get_202_with_no_body() {
    let (app, _cookie, token) = setup_with_token("full").await;
    let res = app
        .oneshot(mcp_req(
            &token,
            serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::ACCEPTED);
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX).await.unwrap();
    assert!(bytes.is_empty(), "a notification must not get a JSON-RPC reply");
}

#[tokio::test]
async fn probe_methods_answer_instead_of_erroring() {
    let (app, _cookie, token) = setup_with_token("full").await;

    for (method, key) in [
        ("prompts/list", "prompts"),
        ("resources/templates/list", "resourceTemplates"),
    ] {
        let body = json_body(
            app.clone()
                .oneshot(mcp_req(&token, rpc(1, method, serde_json::json!({}))))
                .await
                .unwrap(),
        )
        .await;
        assert_eq!(body["result"][key], serde_json::json!([]), "{method} should return an empty list");
    }

    let body = json_body(
        app.clone()
            .oneshot(mcp_req(&token, rpc(2, "ping", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["result"], serde_json::json!({}));

    let body = json_body(
        app.oneshot(mcp_req(&token, rpc(3, "does/not/exist", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["error"]["code"], -32601);
}

#[tokio::test]
async fn batching_and_malformed_json_are_rejected_as_json_rpc_errors() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let body = json_body(
        app.clone()
            .oneshot(mcp_req(
                &token,
                serde_json::json!([rpc(1, "ping", serde_json::json!({}))]),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["error"]["code"], -32600);

    let res = app
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .body(Body::from("{not json"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(json_body(res).await["error"]["code"], -32700);
}

#[tokio::test]
async fn get_and_delete_on_mcp_are_405_not_the_spa_fallback() {
    let (app, _cookie, token) = setup_with_token("full").await;
    for req in [
        Request::get("/mcp")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
        Request::delete("/mcp")
            .header(header::AUTHORIZATION, format!("Bearer {token}"))
            .body(Body::empty())
            .unwrap(),
    ] {
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(res.headers().get(header::ALLOW).unwrap(), "POST");
    }
}

// ----------------------------------------------------------------- auth ---

#[tokio::test]
async fn missing_or_bad_tokens_are_401_with_a_discovery_pointer() {
    let (app, _cookie, token) = setup_with_token("full").await;
    let body = rpc(1, "tools/list", serde_json::json!({}));

    let cases: Vec<Request<Body>> = vec![
        // No Authorization header at all.
        Request::post("/mcp")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap(),
        // Structurally wrong token.
        mcp_req("not-a-finsight-token", body.clone()),
        // Right shape, never issued.
        mcp_req(
            "finsight_pat_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            body.clone(),
        ),
        // Not a bearer scheme.
        Request::post("/mcp")
            .header("content-type", "application/json")
            .header(header::AUTHORIZATION, format!("Basic {token}"))
            .body(Body::from(body.to_string()))
            .unwrap(),
    ];

    for req in cases {
        let res = app.clone().oneshot(req).await.unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        let challenge = res
            .headers()
            .get(header::WWW_AUTHENTICATE)
            .expect("401 must tell the client how to authorize")
            .to_str()
            .unwrap()
            .to_string();
        assert!(challenge.starts_with("Bearer "));
        assert!(
            challenge.contains("/.well-known/oauth-protected-resource"),
            "challenge should point at the protected-resource document, got {challenge}"
        );
    }
}

/// `/mcp` must never honour ambient browser credentials: a cookie-authenticated
/// JSON-RPC endpoint is a CSRF sink, since any page could POST at it.
#[tokio::test]
async fn a_session_cookie_alone_cannot_call_mcp() {
    let (app, cookie, _token) = setup_with_token("full").await;

    let res = app
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header("cookie", &cookie)
                .body(Body::from(rpc(1, "tools/list", serde_json::json!({})).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn a_revoked_token_stops_working_immediately() {
    let (app, cookie, token) = setup_with_token("full").await;

    // Works before revocation.
    let res = app
        .clone()
        .oneshot(mcp_req(&token, rpc(1, "tools/list", serde_json::json!({}))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let listed = json_body(
        app.clone()
            .oneshot(
                Request::get("/api/auth/tokens")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let id = listed[0]["id"].as_str().unwrap().to_string();
    app.clone()
        .oneshot(
            Request::delete(format!("/api/auth/tokens/{id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let res = app
        .oneshot(mcp_req(&token, rpc(2, "tools/list", serde_json::json!({}))))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

/// DNS-rebinding defense: a browser page on another origin can reach this
/// server, and while it can't read the response, it can still fire the request.
#[tokio::test]
async fn a_foreign_origin_header_is_rejected() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let res = app
        .clone()
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::ORIGIN, "https://evil.example")
                .header(header::HOST, "localhost:8674")
                .body(Body::from(rpc(1, "tools/list", serde_json::json!({})).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
    assert_eq!(json_body(res).await["code"], "mcp.bad_origin");

    // A matching Origin (the browser-hosted connector case) is fine.
    let res = app
        .oneshot(
            Request::post("/mcp")
                .header("content-type", "application/json")
                .header(header::AUTHORIZATION, format!("Bearer {token}"))
                .header(header::ORIGIN, "http://localhost:8674")
                .header(header::HOST, "localhost:8674")
                .body(Body::from(rpc(1, "tools/list", serde_json::json!({})).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// --------------------------------------------------------- tool surface ---

/// The parity test: what MCP exposes IS the Copilot's toolset, so the two can
/// never drift. Mirrors the spirit of `tests/parity.rs` for the RPC surface.
#[tokio::test]
async fn tools_list_matches_the_copilot_toolset_exactly() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let body = json_body(
        app.oneshot(mcp_req(&token, rpc(1, "tools/list", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;
    let tools = body["result"]["tools"].as_array().unwrap();
    let listed: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();

    let mut expected: Vec<String> = finsight_agent::reasoning::tools::standard_toolset()
        .definitions()
        .into_iter()
        .map(|d| d.name)
        .collect();
    expected.extend(
        [
            "approve_action_item",
            "execute_action_bundle",
            "get_action_bundle",
            "list_action_bundles",
            "reject_action_item",
        ]
        .map(str::to_string),
    );
    expected.sort();
    assert_eq!(listed, expected);

    // Sorted output keeps the surface stable across restarts (ToolSet is
    // HashMap-backed) so client-side prompt caching actually hits.
    let mut sorted = listed.clone();
    sorted.sort();
    assert_eq!(listed, sorted);

    // Every tool must carry a schema a strict client will accept.
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object", "bad schema for {}", t["name"]);
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
}

#[tokio::test]
async fn a_read_token_cannot_see_or_call_write_tools() {
    let (app, _cookie, token) = setup_with_token("read").await;

    let body = json_body(
        app.clone()
            .oneshot(mcp_req(&token, rpc(1, "tools/list", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;
    let listed: Vec<&str> = body["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();

    for hidden in [
        "draft_set_budget",
        "draft_recategorization",
        "annotate_spending_driver",
        "approve_action_item",
        "execute_action_bundle",
        "reject_action_item",
    ] {
        assert!(!listed.contains(&hidden), "{hidden} must be hidden from a read token");
    }
    // Reads still work, including the read half of bundle management.
    assert!(listed.contains(&"get_net_worth"));
    assert!(listed.contains(&"list_action_bundles"));

    // Hiding is not enough — calling one directly must fail too. The refusal is
    // a transport-level 403 with `error="insufficient_scope"`, not a JSON-RPC
    // error, because that is the form a client can act on: it names the scope
    // to request and triggers the step-up authorization flow instead of
    // dead-ending the user (MCP authorization spec, 2025-11-25).
    for blocked in ["draft_set_budget", "execute_action_bundle"] {
        let res = app
            .clone()
            .oneshot(mcp_req(
                &token,
                rpc(9, "tools/call", serde_json::json!({"name": blocked, "arguments": {}})),
            ))
            .await
            .unwrap();
        assert_eq!(
            res.status(),
            StatusCode::FORBIDDEN,
            "{blocked} must be refused for a read token"
        );
        let challenge = res
            .headers()
            .get("www-authenticate")
            .expect("a scope refusal must carry a WWW-Authenticate challenge")
            .to_str()
            .unwrap()
            .to_string();
        assert!(
            challenge.contains(r#"error="insufficient_scope""#),
            "challenge must name the error so the client can distinguish it from a bad token: {challenge}"
        );
        assert!(
            challenge.contains(r#"scope="full""#),
            "challenge must name the scope to step up to: {challenge}"
        );
        assert!(
            challenge.contains("resource_metadata="),
            "challenge must point at the metadata document: {challenge}"
        );
        let body = json_body(res).await;
        assert_eq!(body["code"], "auth.insufficient_scope");
    }
}

#[tokio::test]
async fn calling_a_read_tool_returns_grounded_data_with_display_strings() {
    let (app, cookie, token) = setup_with_token("full").await;

    // Give the ledger one account so the snapshot has something real in it.
    let res = app
        .clone()
        .oneshot(rpc_req(
            "create_account",
            &cookie,
            serde_json::json!({"input": {
                "owner": "You", "bank": "Test Bank", "type": "Checking",
                "name": "Everyday", "currency": "USD", "color": "#336699",
                "opening_balance_cents": 250_000
            }}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body = call_tool(&app, &token, "get_net_worth", serde_json::json!({})).await;
    let structured = &body["result"]["structuredContent"];
    assert_eq!(structured["ok"], true);
    assert_eq!(body["result"]["isError"], false);
    // Text content mirrors the structured payload so every client can render it.
    assert!(body["result"]["content"][0]["text"].as_str().unwrap().contains("\"ok\""));

    // The `_display` augmentation is what keeps a model from dividing cents by
    // hand — assert it actually reached the wire.
    let dumped = structured.to_string();
    assert!(
        dumped.contains("_display"),
        "cents fields must carry formatted display strings, got {dumped}"
    );
}

#[tokio::test]
async fn a_tool_error_is_data_not_a_protocol_failure() {
    let (app, _cookie, token) = setup_with_token("full").await;

    // Unknown argument: `execute_recoverable` rejects it, and the model is
    // expected to read the error and retry rather than see a transport fault.
    let body = call_tool(
        &app,
        &token,
        "get_net_worth",
        serde_json::json!({"nonsense_argument": 1}),
    )
    .await;
    assert!(body["error"].is_null(), "a tool-level failure is not a JSON-RPC error");
    assert_eq!(body["result"]["isError"], true);
    assert_eq!(body["result"]["structuredContent"]["ok"], false);
    assert!(!body["result"]["structuredContent"]["error"]["code"]
        .as_str()
        .unwrap()
        .is_empty());

    // A genuinely unknown tool IS a protocol error — the client asked for
    // something that was never advertised.
    let body = call_tool(&app, &token, "no_such_tool", serde_json::json!({})).await;
    assert_eq!(body["error"]["code"], -32602);
}

// ------------------------------------------- draft → approve → execute ---

/// The whole point of the feature: an external assistant drafts a change, the
/// user agrees, and the assistant carries it through to applied — with the
/// proposal visible in FinSight's own review surface the entire time.
#[tokio::test]
async fn draft_approve_execute_applies_the_change_end_to_end() {
    let (app, cookie, token) = setup_with_token("full").await;

    let category = json_body(
        app.clone()
            .oneshot(rpc_req(
                "create_category",
                &cookie,
                serde_json::json!({"label": "Groceries", "groupId": null, "color": "#33aa66"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let category_id = category["id"].as_str().unwrap().to_string();

    // 1. Draft. Nothing must change yet.
    let body = call_tool(
        &app,
        &token,
        "draft_set_budget",
        serde_json::json!({
            "category_id": category_id,
            "month": "2026-07",
            "amount_cents": 50_000,
            "rationale": "User asked to cap groceries at $500."
        }),
    )
    .await;
    let structured = &body["result"]["structuredContent"];
    assert_eq!(structured["ok"], true, "draft failed: {structured}");
    let draft = &structured["draft_bundle"];
    assert_eq!(draft["status"], "pending_approval");
    let bundle_id = draft["bundleId"].as_str().unwrap().to_string();
    let item_id = draft["items"][0]["itemId"].as_str().unwrap().to_string();
    assert_eq!(draft["items"][0]["actionKind"], "set_budget");

    // The proposal is visible in the app's own review surface, not just to MCP.
    let bundles = json_body(
        app.clone()
            .oneshot(rpc_req(
                "list_action_bundles",
                &cookie,
                serde_json::json!({"statusFilter": "pending", "sessionId": null, "limit": 25}),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(bundles.as_array().unwrap().len(), 1);
    assert_eq!(bundles[0]["id"], bundle_id.as_str());
    assert!(
        bundles[0]["providerId"]
            .as_str()
            .unwrap()
            .starts_with("mcp:"),
        "provenance must record which MCP token drafted this, got {}",
        bundles[0]["providerId"]
    );
    assert_eq!(bundles[0]["modelId"], "Claude Desktop", "the connected client is named");

    // 2. Approve — still not applied.
    let body = call_tool(
        &app,
        &token,
        "approve_action_item",
        serde_json::json!({"bundle_id": bundle_id, "item_id": item_id}),
    )
    .await;
    assert_eq!(body["result"]["structuredContent"]["ok"], true);
    assert!(body["result"]["structuredContent"]["data"]["note"]
        .as_str()
        .unwrap()
        .contains("NOT yet applied"));

    // 3. Execute — now it lands.
    let body = call_tool(
        &app,
        &token,
        "execute_action_bundle",
        serde_json::json!({"bundle_id": bundle_id}),
    )
    .await;
    let data = &body["result"]["structuredContent"]["data"];
    assert_eq!(body["result"]["structuredContent"]["ok"], true, "execute failed: {data}");
    assert_eq!(data["succeeded"], 1, "execution summary: {data}");
    assert_eq!(data["failed"], 0);

    // Verify through a completely separate read path: the budget is really set.
    let budgets = call_tool(&app, &token, "get_budgets", serde_json::json!({})).await;
    let dumped = budgets["result"]["structuredContent"].to_string();
    assert!(
        dumped.contains("50000") || dumped.contains("500.00"),
        "the $500 budget should be readable after execution, got {dumped}"
    );
}

/// Approve/execute are limited to bundles this interface created. The user
/// authorized a model to carry its OWN proposals through — not to apply
/// something the in-app Copilot suggested while they were still deciding.
#[tokio::test]
async fn mcp_cannot_approve_or_execute_an_in_app_bundle() {
    let (state, _dir) = fresh_state();
    let app = build_router(state.clone(), &test_ui_dir());
    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/setup")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"username":"alice","password":"hunter22-plus"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let cookie = cookie_from(&res);
    let token = mint_token(&app, &cookie, "full").await;

    // Force a runtime to exist, then write a bundle that did NOT come from MCP,
    // exactly as an in-app Copilot turn would.
    app.clone()
        .oneshot(rpc_req("list_accounts", &cookie, serde_json::json!({})))
        .await
        .unwrap();
    let user_id = state.users.list_users().unwrap()[0].id.clone();
    let rt = state
        .registry
        .get_or_bootstrap(&state.data_dir, &user_id, "")
        .await
        .expect("runtime should already be bootstrapped");
    let db = (*rt.api.db).clone();
    let (bundle_id, item_id) = finsight_core::repos::run(&db, move |conn| {
        let mut b = finsight_core::repos::copilot_actions::insert_bundle(
            conn, None, "In-app proposal", "summary", "rationale", 0.9,
            Some("openai"), Some("gpt-mini"),
        )?;
        let item = finsight_core::repos::copilot_actions::insert_item(
            conn, &b.id, "set_budget", "{}", "r", 0.9, 0,
        )?;
        b.items.push(item.clone());
        Ok::<_, finsight_core::CoreError>((b.id, item.id))
    })
    .await
    .unwrap();

    for (tool, args) in [
        (
            "approve_action_item",
            serde_json::json!({"bundle_id": bundle_id, "item_id": item_id}),
        ),
        ("execute_action_bundle", serde_json::json!({"bundle_id": bundle_id})),
    ] {
        let body = call_tool(&app, &token, tool, args).await;
        let structured = &body["result"]["structuredContent"];
        assert_eq!(structured["ok"], false, "{tool} must refuse a non-MCP bundle");
        assert_eq!(structured["error"]["code"], "not_an_mcp_bundle");
        assert_eq!(body["result"]["isError"], true);
    }

    // Reading it is still allowed — "what's waiting for me?" is a question.
    let body = call_tool(&app, &token, "list_action_bundles", serde_json::json!({})).await;
    assert_eq!(body["result"]["structuredContent"]["ok"], true);
}

/// Approving is meant to express "the user agreed, in THIS conversation". A
/// second connected assistant never witnessed that agreement, so it must not be
/// able to claim it — even though both tokens belong to the same user and both
/// have full scope.
#[tokio::test]
async fn one_connected_client_cannot_approve_anothers_draft() {
    let (app, cookie, claude_token) = setup_with_token("full").await;
    let chatgpt_token = mint_token(&app, &cookie, "full").await;

    let category = json_body(
        app.clone()
            .oneshot(rpc_req(
                "create_category",
                &cookie,
                serde_json::json!({"label": "Groceries", "groupId": null, "color": "#33aa66"}),
            ))
            .await
            .unwrap(),
    )
    .await;
    let category_id = category["id"].as_str().unwrap().to_string();

    // Claude drafts a change the user has not agreed to yet.
    let draft = call_tool(
        &app,
        &claude_token,
        "draft_set_budget",
        serde_json::json!({
            "category_id": category_id, "month": "2026-07", "amount_cents": 50_000
        }),
    )
    .await;
    let bundle = &draft["result"]["structuredContent"]["draft_bundle"];
    let bundle_id = bundle["bundleId"].as_str().unwrap().to_string();
    let item_id = bundle["items"][0]["itemId"].as_str().unwrap().to_string();

    // The other client can SEE it (answering "what's pending?" is a question)...
    let listed = call_tool(&app, &chatgpt_token, "list_action_bundles", serde_json::json!({})).await;
    assert_eq!(listed["result"]["structuredContent"]["ok"], true);

    // ...but cannot act on it.
    for (tool, args) in [
        (
            "approve_action_item",
            serde_json::json!({"bundle_id": bundle_id, "item_id": item_id}),
        ),
        (
            "reject_action_item",
            serde_json::json!({"bundle_id": bundle_id, "item_id": item_id}),
        ),
        ("execute_action_bundle", serde_json::json!({"bundle_id": bundle_id})),
    ] {
        let body = call_tool(&app, &chatgpt_token, tool, args).await;
        let structured = &body["result"]["structuredContent"];
        assert_eq!(structured["ok"], false, "{tool} must refuse another client's bundle");
        assert_eq!(structured["error"]["code"], "not_your_bundle", "for {tool}");
    }

    // Nothing was applied.
    let budgets = call_tool(&app, &claude_token, "get_budgets", serde_json::json!({})).await;
    assert_eq!(
        budgets["result"]["structuredContent"]["data"]["budgets"],
        serde_json::json!([]),
        "a refused approval must not have changed the ledger"
    );

    // The drafting client itself still can.
    let body = call_tool(
        &app,
        &claude_token,
        "approve_action_item",
        serde_json::json!({"bundle_id": bundle_id, "item_id": item_id}),
    )
    .await;
    assert_eq!(body["result"]["structuredContent"]["ok"], true);
}

#[tokio::test]
async fn bundle_tools_validate_their_arguments() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let body = call_tool(&app, &token, "get_action_bundle", serde_json::json!({})).await;
    assert_eq!(
        body["result"]["structuredContent"]["error"]["code"],
        "missing_required_argument"
    );

    let body = call_tool(
        &app,
        &token,
        "get_action_bundle",
        serde_json::json!({"bundle_id": "nope"}),
    )
    .await;
    assert_eq!(body["result"]["structuredContent"]["error"]["code"], "bundle_not_found");
}

// ------------------------------------------------------------ discovery ---

#[tokio::test]
async fn oauth_metadata_documents_are_public_and_well_formed() {
    let (app, _cookie, _token) = setup_with_token("full").await;

    let res = app
        .clone()
        .oneshot(
            Request::get("/.well-known/oauth-protected-resource")
                .header(header::HOST, "fin.example.com")
                .header("x-forwarded-proto", "https")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK, "discovery must work unauthenticated");
    let body = json_body(res).await;
    assert_eq!(body["resource"], "https://fin.example.com/mcp");
    assert_eq!(body["authorization_servers"][0], "https://fin.example.com");

    let body = json_body(
        app.clone()
            .oneshot(
                Request::get("/.well-known/oauth-authorization-server")
                    .header(header::HOST, "fin.example.com")
                    .header("x-forwarded-proto", "https")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(body["issuer"], "https://fin.example.com");
    assert_eq!(
        body["authorization_endpoint"],
        "https://fin.example.com/oauth/authorize"
    );
    assert_eq!(body["code_challenge_methods_supported"][0], "S256");
    assert_eq!(body["token_endpoint_auth_methods_supported"][0], "none");

    // Clients also probe the path-inserted form when the resource has a path.
    let res = app
        .oneshot(
            Request::get("/.well-known/oauth-protected-resource/mcp")
                .header(header::HOST, "fin.example.com")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

// ------------------------------------------------------- MCP Apps UI ---

#[tokio::test]
async fn ui_resources_are_listed_and_readable() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let body = json_body(
        app.clone()
            .oneshot(mcp_req(&token, rpc(1, "resources/list", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;
    let resources = body["result"]["resources"].as_array().expect("a resources array");
    assert!(!resources.is_empty(), "widgets must be advertised to render at all");

    for r in resources {
        let uri = r["uri"].as_str().unwrap();
        assert!(uri.starts_with("ui://"), "UI resources use the ui:// scheme, got {uri}");
        assert_eq!(
            r["mimeType"], "text/html;profile=mcp-app",
            "the mime type is what marks this as an MCP App rather than a plain HTML blob"
        );

        // Every advertised resource must actually be readable: a tool pointing
        // at a URI that 404s renders as a broken frame in the host.
        let read = json_body(
            app.clone()
                .oneshot(mcp_req(
                    &token,
                    rpc(2, "resources/read", serde_json::json!({"uri": uri})),
                ))
                .await
                .unwrap(),
        )
        .await;
        let content = &read["result"]["contents"][0];
        assert_eq!(content["uri"], *uri);
        let html = content["text"].as_str().unwrap_or_default();
        assert!(html.contains("id=\"root\""), "{uri} must render into a root node");
        assert!(
            html.contains("ui/notifications/tool-result"),
            "{uri} must listen for the host's tool-result notification"
        );
        // Self-contained is a hard requirement, not a preference: a host CSP
        // that blocks external fetches would otherwise render a blank card.
        assert!(
            !html.contains("http://") && !html.contains("https://"),
            "{uri} must not reference any external origin"
        );
    }

    let missing = json_body(
        app.clone()
            .oneshot(mcp_req(
                &token,
                rpc(3, "resources/read", serde_json::json!({"uri": "ui://finsight/nope.html"})),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(missing["error"]["code"], -32002);
}

#[tokio::test]
async fn tools_with_a_widget_point_at_a_resource_that_exists() {
    let (app, _cookie, token) = setup_with_token("full").await;

    let listed = json_body(
        app.clone()
            .oneshot(mcp_req(&token, rpc(1, "resources/list", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;
    let known: Vec<String> = listed["result"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap().to_string())
        .collect();

    let tools = json_body(
        app.clone()
            .oneshot(mcp_req(&token, rpc(2, "tools/list", serde_json::json!({}))))
            .await
            .unwrap(),
    )
    .await;

    let mut with_widget = 0;
    for t in tools["result"]["tools"].as_array().unwrap() {
        let Some(meta) = t.get("_meta") else { continue };
        with_widget += 1;
        let uri = meta["ui"]["resourceUri"].as_str().expect("_meta.ui.resourceUri");
        assert!(
            known.contains(&uri.to_string()),
            "{} points at {uri}, which resources/list does not offer",
            t["name"]
        );
        // ChatGPT reads a different field for the same thing; emitting only one
        // renders in one product and silently not in the other.
        assert_eq!(
            meta["openai/outputTemplate"].as_str(),
            Some(uri),
            "the ChatGPT alias must match the MCP Apps field"
        );
    }
    assert!(with_widget > 0, "at least one tool should offer a widget");
}
