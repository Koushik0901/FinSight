#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tracing_subscriber::fmt::init();

    let builder = finsight_app::configure_app(tauri::Builder::default());
    if let Err(e) = builder.run(tauri::generate_context!()) {
        eprintln!("fatal: {e}");
        std::process::exit(1);
    }
}
