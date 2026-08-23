//! Exports the OpenAPI specification for the frontend.
//! Invoked by `cargo run -p finsight-openapi --bin export_openapi`.
//! Writes `openapi.json` at the workspace root and at `ui/src/api/openapi.json`
//! (the latter is consumed by `openapi-typescript` to generate the typed client).
//!
//! The single source of truth is `finsight_openapi::build_openapi()` — the same
//! function the server's `GET /api/openapi.json` handler returns.

fn main() -> anyhow::Result<()> {
    let spec = finsight_openapi::build_openapi();
    let json = serde_json::to_string_pretty(&spec)?;
    // Workspace-relative paths — must be run from the repo root, like the old
    // `export_bindings` binary (the plan keeps that invariant so CI + dev share it).
    let root = "openapi.json";
    let ui = "ui/src/api/openapi.json";
    std::fs::write(root, &json)?;
    println!("openapi written to {root}");
    // ui/src/api may not exist in a fresh checkout that hasn't built the frontend
    // yet — ensure the directory exists rather than failing the export.
    if let Some(parent) = std::path::Path::new(ui).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(ui, &json)?;
    println!("openapi written to {ui}");
    Ok(())
}
