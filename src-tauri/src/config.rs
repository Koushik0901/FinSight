//! Server-URL storage for the thin desktop shell (Phase 4). Deliberately NOT
//! part of the generated bindings.ts — these 3 commands only exist for the
//! shell's own local ConnectScreen, called via raw `invoke()`, not the shared
//! command surface the rest of the app (browser/PWA/post-navigate shell) uses
//! over HTTP. finsight_core::keychain is already generic over (service, user)
//! — reused as-is, no changes to that module.
const SERVICE: &str = "com.finsight.desktop";
const USER: &str = "server_url";

#[tauri::command]
pub fn get_server_url() -> Result<Option<String>, String> {
    finsight_core::keychain::get_key(SERVICE, USER).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_server_url(url: String) -> Result<(), String> {
    finsight_core::keychain::set_key(SERVICE, USER, &url).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_server_url() -> Result<(), String> {
    finsight_core::keychain::delete_key(SERVICE, USER).map_err(|e| e.to_string())
}
