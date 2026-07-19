//! End-to-end check for `get_account_balance_timeline` over the real HTTP
//! stack (`build_router` + `oneshot`, no socket): auth → dispatch → api → core
//! → JSON back out.
//!
//! The unit tests in `finsight-core` prove the reconstruction arithmetic and the
//! ones in `finsight-agent` prove the Copilot tool. Neither exercises the wire,
//! which is where this command's two riskiest details live: the response is
//! camelCase (most types in this codebase are, but `Transaction`/`NewAccount`
//! are NOT, so the boundary is easy to get wrong) and `since` is an optional arg
//! that has to survive being sent as JSON `null`.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use finsight_server::router::build_router;
use finsight_server::state::ServerState;
use std::path::PathBuf;
use std::sync::Arc;
use tower::util::ServiceExt;

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
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

fn cookie_from(res: &axum::response::Response) -> String {
    let raw = res
        .headers()
        .get(header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    raw.split(';').next().unwrap().to_string()
}

fn rpc(cmd: &str, cookie: &str, payload: serde_json::Value) -> Request<Body> {
    Request::post(format!("/api/rpc/{cmd}"))
        .header("content-type", "application/json")
        .header("cookie", cookie)
        .body(Body::from(payload.to_string()))
        .unwrap()
}

#[tokio::test]
async fn balance_timeline_round_trips_over_http_with_camelcase_keys() {
    let (state, _dir) = fresh_state();
    let app = build_router(state, &test_ui_dir());

    let res = app
        .clone()
        .oneshot(
            Request::post("/api/auth/setup")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"username": "alice", "password": "correct horse battery"})
                        .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let cookie = cookie_from(&res);

    // `NewAccount` has no rename_all, so its keys are snake_case.
    let res = app
        .clone()
        .oneshot(rpc(
            "create_account",
            &cookie,
            serde_json::json!({"input": {
                "owner": "You",
                "bank": "Test Bank",
                "type": "Savings",
                "name": "Car Savings",
                "currency": "USD",
                "color": "#336699",
                "opening_balance_cents": 100_000
            }}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let account_id = json_body(res).await["id"].as_str().unwrap().to_string();

    // Rise to $10,000, then fall back to $3,000. The peak lands on a day no
    // stored snapshot records, which is the case this command exists to handle.
    for (date, amount) in [
        ("2024-02-01", 500_000),
        ("2024-05-01", 400_000),
        ("2024-08-01", -700_000),
    ] {
        let res = app
            .clone()
            .oneshot(rpc(
                "create_transaction",
                &cookie,
                serde_json::json!({"input": {
                    "account_id": account_id,
                    "posted_at": format!("{date}T12:00:00Z"),
                    "amount_cents": amount,
                    "merchant_raw": "Transfer",
                    "category_id": null,
                    "notes": null,
                    "status": "cleared",
                    "imported_id": null,
                    "source": null,
                    "raw_synced_data": null,
                    "pending": false,
                    "external_tx_id": null,
                    "external_account_id": null
                }}),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK, "create_transaction failed");
    }

    // `since: null` — the optional arg has to survive the wire as JSON null.
    let res = app
        .clone()
        .oneshot(rpc(
            "get_account_balance_timeline",
            &cookie,
            serde_json::json!({"accountId": account_id, "since": null}),
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = json_body(res).await;

    assert_eq!(body["accountName"], "Car Savings");
    assert_eq!(body["reconstructable"], true);
    assert_eq!(body["currentCents"], 300_000);
    assert_eq!(body["peak"]["balanceCents"], 1_000_000);
    assert_eq!(body["peak"]["date"], "2024-05-01");
    // A real opening balance was entered, so the amounts are anchored. The enum
    // has to arrive as a camelCase string the frontend can switch on.
    assert_eq!(body["anchor"], "anchoredOpening");
    assert_eq!(body["earliestTxnDate"], "2024-02-01");

    // Windowing past the peak must exclude it, and carry the running balance in
    // rather than restarting from the opening.
    let res = app
        .clone()
        .oneshot(rpc(
            "get_account_balance_timeline",
            &cookie,
            serde_json::json!({"accountId": account_id, "since": "2024-06-01"}),
        ))
        .await
        .unwrap();
    let windowed = json_body(res).await;
    assert_eq!(windowed["peak"]["date"], "2024-06-01");
    assert_eq!(windowed["peak"]["balanceCents"], 1_000_000);
    assert_eq!(windowed["points"][0]["date"], "2024-06-01");
}
