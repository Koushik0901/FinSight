#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Thin desktop shell (Phase 4): no local command surface, no local database.
// Reads a server URL from the OS keychain (crates/finsight_app is NOT used
// here — it exists only for the `export_bindings` bin's TypeScript codegen).
// On first launch (no stored URL) the bundled ui/dist app shows its own
// ConnectScreen (gated by DesktopConnectGate, itself gated on isTauriRuntime()
// — see ui/src/utils/runtime.ts); once a URL is set, the window navigates
// there directly and behaves exactly like the browser/PWA from that point on.

mod config;

use tauri::menu::{CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::Manager;
use tauri_plugin_autostart::ManagerExt;

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
        // Opt-in launch-at-login. `--minimized` is the argument the shell reads
        // to know an OS-triggered launch should stay in the tray rather than
        // steal focus. Registration is toggled from the tray, never enabled by
        // default — an app that adds itself to startup unasked is a bad
        // citizen.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .invoke_handler(tauri::generate_handler![
            config::get_server_url,
            config::set_server_url,
            config::clear_server_url,
        ])
        .setup(|app| {
            let change_server =
                MenuItemBuilder::with_id("change-server", "Change Server…").build(app)?;
            // Reflects the real registration state, so the tick is right even
            // after the user toggled it in a previous session.
            let launch_at_login = CheckMenuItemBuilder::with_id("launch-at-login", "Launch at startup")
                .checked(app.autolaunch().is_enabled().unwrap_or(false))
                .build(app)?;
            let quit = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .items(&[&change_server, &launch_at_login, &quit])
                .build()?;

            // The menu-event closure is 'static, so it owns a clone of the
            // checkbox to reflect the new state back after a toggle.
            let launch_at_login_for_events = launch_at_login.clone();

            TrayIconBuilder::new()
                .menu(&menu)
                .icon(app.default_window_icon().cloned().unwrap())
                .on_menu_event(move |app, event| match event.id().as_ref() {
                    "change-server" => {
                        // Clear the stored URL, then fully restart the process.
                        // The entire boot sequence (main.rs -> main window
                        // created fresh at the app's own default WebviewUrl ->
                        // ui/dist's main.tsx boot -> DesktopConnectGate's
                        // effect) runs from scratch, which naturally shows
                        // ConnectScreen again since get_server_url() is now
                        // None. Simpler and more robust than navigating the
                        // live window back to the app's own origin by hand.
                        // Go through config::clear_server_url() rather than
                        // re-stating the keychain (service, user) literals — it
                        // owns those consts, and a duplicated copy here would
                        // silently stop matching if they ever change, leaving
                        // "Change Server…" deleting a key nobody writes (the
                        // shell would just navigate back to the stale server
                        // with no other UI to clear it).
                        let _ = config::clear_server_url();
                        app.restart();
                    }
                    "launch-at-login" => {
                        // Register or unregister with the OS, then let the
                        // checkbox settle to whatever actually took effect —
                        // if the toggle failed, the tick should not lie about
                        // it. Re-reading `is_enabled` is the source of truth.
                        let manager = app.autolaunch();
                        let now_enabled = manager.is_enabled().unwrap_or(false);
                        let _ = if now_enabled {
                            manager.disable()
                        } else {
                            manager.enable()
                        };
                        let _ = launch_at_login_for_events
                            .set_checked(manager.is_enabled().unwrap_or(now_enabled));
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

            // An OS-triggered launch-at-login run carries `--minimized`: start
            // in the tray rather than stealing focus, matching what people
            // expect from an always-on background utility. The window's default
            // is visible, so this hides it; a tray click brings it back. A
            // manual launch has no such flag and opens normally.
            if std::env::args().any(|a| a == "--minimized") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running FinSight desktop shell");
}
