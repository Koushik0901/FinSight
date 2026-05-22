#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt::init();
    // Real lifecycle (DB, single-instance, commands) wired in Task 13.
    tauri::Builder::default()
        .setup(|_app| Ok(()))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
