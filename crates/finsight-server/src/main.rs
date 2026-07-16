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
    let port: u16 = std::env::var("FINSIGHT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8674);
    let state = state::ServerState::bootstrap(&data_dir).await?;
    let app = router::build_router(state);
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await?;
    tracing::info!("finsight-server listening on http://localhost:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
