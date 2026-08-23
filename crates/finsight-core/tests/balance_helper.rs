use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../")
}

fn walk_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_files(&path, out);
            } else if path.extension().map(|e| e == "rs").unwrap_or(false) {
                out.push(path);
            }
        }
    }
}

fn line_contains_case_source_simplefin(line: &str) -> bool {
    let Some(case_idx) = line.find("CASE") else {
        return false;
    };
    let Some(source_idx) = line[case_idx..].find("source").map(|i| i + case_idx) else {
        return false;
    };
    let Some(when_idx) = line[source_idx..].find("WHEN").map(|i| i + source_idx) else {
        return false;
    };
    line[when_idx..].contains("simplefin")
}

fn contains_case_source_simplefin(content: &str) -> bool {
    content.lines().any(line_contains_case_source_simplefin)
}

fn contains_order_by_case_source(content: &str) -> bool {
    content
        .lines()
        .any(|l| l.contains("ORDER BY CASE") && l.contains("source"))
}

#[test]
fn case_fragment_single_sourced() {
    let root = workspace_root();
    let crates_dir = root.join("crates");
    let mut files = Vec::new();
    walk_files(&crates_dir, &mut files);
    let mut dup1 = Vec::new();
    let mut dup2 = Vec::new();
    for p in &files {
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name == "balance.rs" || name == "balance_helper.rs" {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(p) {
            if contains_case_source_simplefin(&content) {
                dup1.push(p.clone());
            }
            if contains_order_by_case_source(&content) {
                dup2.push(p.clone());
            }
        }
    }
    assert!(
        dup1.is_empty(),
        "CASE still duplicated outside balance.rs: {:?}",
        dup1
    );
    assert!(
        dup2.is_empty(),
        "ORDER BY CASE still duplicated outside balance.rs: {:?}",
        dup2
    );
    // Sanity: balance.rs must contain the fragment
    let bal_path = root.join("crates/finsight-core/src/repos/balance.rs");
    let bal_content = std::fs::read_to_string(&bal_path).unwrap_or_default();
    assert!(
        contains_case_source_simplefin(&bal_content),
        "balance.rs should contain CASE source fragment"
    );
}
