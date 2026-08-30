use finsight_api::ApiState;
use std::sync::Arc;

fn test_state(db: finsight_core::Db) -> (tempfile::TempDir, ApiState) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let state = ApiState::new(db, path, Arc::new(|_| {}));
    (dir, state)
}

#[tokio::test]
async fn copilot_safe_to_spend_equals_dashboard() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    let (_tmp, state) = test_state(db);
    // Dashboard path: ApiState -> cashflow::build_forecast(SafetyConservative)
    let dashboard = finsight_api::commands::cashflow::get_cashflow_forecast(
        &state,
        Some(30),
        None,
        None,
        None,
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        dashboard.safe_to_spend_cents, copilot.safe_to_spend_cents,
        "dashboard safe_to_spend != copilot safe_to_spend"
    );
}
