#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

fn main() {
    tracing_subscriber::fmt::init();

    let builder = finsight_app::configure_app(tauri::Builder::default());
    let app = builder
        .build(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("fatal: {e}");
            std::process::exit(1);
        });
    app.run(|app_handle, event| {
        // Checkpoint the WAL on a clean exit so the next launch starts from a
        // truncated WAL — less un-checkpointed state to replay/recover, and the
        // WAL never lingers at the size of the whole DB between sessions.
        if let tauri::RunEvent::ExitRequested { .. } = event {
            if let Some(state) = app_handle.try_state::<finsight_app::AppState>() {
                let _ = state.api.db.checkpoint();
            }
        }
    });
}
