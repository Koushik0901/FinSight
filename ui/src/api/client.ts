// Re-export the typed commands and types from the generated bindings.
// All Tauri IPC access in the UI should route through this module so the
// bindings file remains a generated implementation detail.
export * from "./bindings";
