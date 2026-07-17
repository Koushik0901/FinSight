#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin desktop shell (Phase 4): no local command surface, no local database.
// Reads a server URL from the OS keychain (crates/finsight_app is NOT used
// here — it exists only for the `export_bindings` bin's TypeScript codegen).
// On first launch (no stored URL) the bundled ui/dist app shows its own
// ConnectScreen (gated by DesktopConnectGate, itself gated on isTauriRuntime()
// — see ui/src/utils/runtime.ts); once a URL is set, the window navigates
// there directly and behaves exactly like the browser/PWA from that point on.

mod config;

use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Focus the existing window instead of opening a second one — no
            // local-DB-lock reason to enforce this anymore, but two windows
            // of the same shell is still bad UX.
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![
            config::get_server_url,
            config::set_server_url,
            config::clear_server_url,
        ])
        .setup(|app| {
            let change_server =
                MenuItemBuilder::with_id("change-server", "Change Server…").build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&change_server, &quit]).build()?;

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().cloned().unwrap())
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "change-server" => {
                        // Clear the stored URL, then fully restart the process.
                        // The entire boot sequence (main.rs -> main window
                        // created fresh at the app's own default WebviewUrl ->
                        // ui/dist's main.tsx boot -> DesktopConnectGate's
                        // effect) runs from scratch, which naturally shows
                        // ConnectScreen again since get_server_url() is now
                        // None. Simpler and more robust than navigating the
                        // live window back to the app's own origin by hand.
                        let _ = finsight_core::keychain::delete_key(
                            "com.finsight.desktop",
                            "server_url",
                        );
                        app.restart();
                    }
                    "quit" => std::process::exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FinSight desktop shell");
}
