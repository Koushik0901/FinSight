use finsight_server::{router, state};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();
    let data_dir = std::path::PathBuf::from(
        std::env::var("FINSIGHT_DATA_DIR").unwrap_or_else(|_| "./data".into()),
    );
    let ui_dir = std::path::PathBuf::from(
        std::env::var("FINSIGHT_UI_DIR").unwrap_or_else(|_| "ui/dist".into()),
    );
    let port: u16 = std::env::var("FINSIGHT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8674);
    let state = state::ServerState::bootstrap(&data_dir)?;

    // Idle-eviction sweep: drop per-user runtimes (DB pool, agent, sync loop)
    // that have had no request in 30 minutes. The session itself (and its
    // unwrapped key) is untouched — the next request just rebuilds the
    // runtime. Never logs key material, only user ids.
    {
        let registry_state = state.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                ticker.tick().await;
                let evicted = registry_state
                    .registry
                    .evict_idle(std::time::Duration::from_secs(1800));
                if !evicted.is_empty() {
                    tracing::info!(user_ids = ?evicted, "evicted idle per-user runtimes");
                }
            }
        });
    }

    let app = router::build_router(state, &ui_dir);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("finsight-server listening on http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
