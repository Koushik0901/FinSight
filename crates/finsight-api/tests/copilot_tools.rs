use finsight_api::ApiState;
use std::sync::Arc;

fn test_state(db: finsight_core::Db) -> ApiState {
    let dir = tempfile::tempdir().unwrap();
    // Leak dir to keep temp path alive for ApiState's data_dir; test owns db, dir will be dropped but ApiState keeps PathBuf copy.
    // Use a persistent temp dir that lives as long as ApiState? Clone path before drop.
    let path = dir.path().to_path_buf();
    // Prevent dir drop deleting underlying; we need to keep dir alive via forgetting? Instead use std::path that exists even after drop? Simpler: create ApiState with dir.path() and keep dir leaked.
    std::mem::forget(dir);
    ApiState::new(db, path, Arc::new(|_| {}))
}

#[tokio::test]
async fn copilot_safe_to_spend_equals_dashboard() {
    let (_dir, db) = finsight_core::testing::migrated_db();
    let state = test_state(db);
    // Dashboard path: ApiState -> cashflow::build_forecast(SafetyConservative)
    let dashboard =
        finsight_api::commands::cashflow::get_cashflow_forecast(&state, Some(30), None, None, None)
            .await
            .unwrap();
    let copilot = finsight_api::commands::copilot::get_safe_to_spend(&state, Some(30), None, None)
        .await
        .unwrap();
    assert_eq!(
        dashboard.safe_to_spend_cents, copilot.safe_to_spend_cents,
        "dashboard safe_to_spend != copilot safe_to_spend"
    );
}
