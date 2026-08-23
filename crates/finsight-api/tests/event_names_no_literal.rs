/// Guard: no literal wire-event strings outside the single source.
///
/// Mirrors the CI check `rg -n '"copilot-stream-frame"' --glob '!event_names*'`
/// from the cleanup plan. If this fails, a producer or consumer spelled a
/// wire name as a string literal instead of `event_names::X` / `eventNames.ts`.
#[test]
fn no_literal_event_names_outside_single_source() {
    // Resolve workspace root from this crate's manifest so the test works
    // regardless of where `cargo test` is invoked from (crate dir vs workspace root).
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    // Use rg if available (faster, respects .gitignore), else fall back to
    // a manual walk that skips the same allow-list.
    let result = std::process::Command::new("rg")
        .args([
            "-n",
            // search for the canonical wire name with quotes to avoid matching
            // comments that mention it without quotes or CSS class names.
            "\"copilot-stream-frame\"",
            "crates/",
            "ui/src/",
            "--glob",
            "!event_names.*",
            "--glob",
            "!eventNames.*",
            "--glob",
            "!event_names_no_literal.rs",
        ])
        .current_dir(workspace_root)
        .output();

    if let Ok(out) = result {
        let stdout = String::from_utf8_lossy(&out.stdout);
        // rg exits 0 when matches found, 1 when none. stdout is the load-bearing signal.
        // Filter out known allowed litters: docs comments are not in crates/ui, so only
        // the single-source files remain excluded via globs above.
        let hits: Vec<&str> = stdout
            .lines()
            // allow the sink definition file itself when it is still sink.rs (pre-split)
            // — the split moves it to sink/event_names.rs which is already excluded.
            // To keep the guard useful before the split, we ignore the definition site
            // `pub const COPILOT_STREAM_FRAME: &str = "copilot-stream-frame"` line.
            .filter(|line| !line.contains("pub const COPILOT_STREAM_FRAME"))
            .filter(|line| !line.trim().is_empty())
            .collect();
        assert!(
            hits.is_empty(),
            "literal \"copilot-stream-frame\" remains outside single source (event_names.* / eventNames.*):\n{}",
            hits.join("\n")
        );
        return;
    }

    // Fallback: manual walk without rg.
    let mut hits = Vec::new();
    for root in ["crates", "ui/src"] {
        collect_hits(std::path::Path::new(root), &mut hits);
    }
    assert!(
        hits.is_empty(),
        "literal \"copilot-stream-frame\" remains outside single source:\n{}",
        hits.join("\n")
    );
}

fn collect_hits(dir: &std::path::Path, hits: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // allow-list: the single source files + this test itself
        if name.starts_with("event_names") || name.starts_with("eventNames") || name == "event_names_no_literal.rs" {
            continue;
        }
        if path.is_dir() {
            // skip target, node_modules, .git, dist
            if name == "target" || name == "node_modules" || name == ".git" || name == "dist" {
                continue;
            }
            collect_hits(&path, hits);
            continue;
        }
        // only inspect Rust and TS files
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !matches!(ext, "rs" | "ts" | "tsx") {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&path) {
            for (i, line) in text.lines().enumerate() {
                if line.contains("\"copilot-stream-frame\"") && !line.contains("pub const COPILOT_STREAM_FRAME") {
                    hits.push(format!("{}:{}: {}", path.display(), i + 1, line.trim()));
                }
            }
        }
    }
}
