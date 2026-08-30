use finsight_providers::enable_banking::{
    fetch_enable_data, fetch_enable_data_with_base_url, EnableBankingClient,
};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn enable_banking_fetch_isolates_per_user() {
    // stub removed (C2): literal tokens now hit network and fail rather than returning stubbed accounts
    let a = fetch_enable_data("token-a").await.unwrap_err();
    let b = fetch_enable_data("token-b").await.unwrap_err();
    use finsight_providers::ProviderError;
    for err in [a, b] {
        assert!(
            matches!(
                err,
                ProviderError::Forbidden
                    | ProviderError::ServerError(_)
                    | ProviderError::Http(_)
                    | ProviderError::Internal(_)
            ),
            "literal token should error via network, got {err:?}"
        );
    }
}

#[tokio::test]
async fn enable_banking_fetch_isolates_via_bearer_mock() {
    // same guarantee via real HTTP + per-bearer mock (no stub)
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", "Bearer tok-a-mock"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accounts": [{"uid": "acc-mock-a", "name": "A Checking", "currency": "EUR"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", "Bearer tok-b-mock"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accounts": [{"uid": "acc-mock-b", "name": "B Savings", "currency": "EUR"}]
        })))
        .mount(&server)
        .await;
    let base = server.uri();
    let a = fetch_enable_data_with_base_url("tok-a-mock", &base)
        .await
        .unwrap();
    let b = fetch_enable_data_with_base_url("tok-b-mock", &base)
        .await
        .unwrap();
    assert_ne!(a.accounts[0].id, b.accounts[0].id);
    assert_eq!(a.accounts[0].id, "acc-mock-a");
    assert_eq!(b.accounts[0].id, "acc-mock-b");
}

#[tokio::test]
async fn enable_banking_client_isolates_per_token_with_wiremock() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", "Bearer token-a"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accounts": [{"id": "acc-a-1", "name": "A", "currency": "EUR"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/accounts"))
        .and(header("Authorization", "Bearer token-b"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "accounts": [{"id": "acc-b-1", "name": "B", "currency": "EUR"}]
        })))
        .mount(&server)
        .await;
    let base = server.uri();
    let a = EnableBankingClient::with_base_url("token-a", &base)
        .unwrap()
        .list_accounts()
        .await
        .unwrap();
    let b = EnableBankingClient::with_base_url("token-b", &base)
        .unwrap()
        .list_accounts()
        .await
        .unwrap();
    assert_ne!(a[0].id, b[0].id);
}
