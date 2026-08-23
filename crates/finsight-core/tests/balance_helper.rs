use std::path::PathBuf;
use std::process::Command;

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for this test is crates/finsight-core
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.join("../../")
}

#[test]
fn case_fragment_single_sourced() {
    let root = workspace_root();
    // Broad pattern: any balance source precedence CASE must live only in balance.rs
    let out = Command::new("rg")
        .args([
            "-n",
            "CASE.*source.*WHEN.*simplefin",
            root.join("crates").to_str().unwrap(),
            "--glob",
            "!balance.rs",
            "--glob",
            "!balance_helper.rs",
        ])
        .output()
        .unwrap();
    assert!(
        out.stdout.is_empty(),
        "CASE still duplicated outside balance.rs:\n{}",
        String::from_utf8_lossy(&out.stdout)
    );
    // Also guard the narrow ORDER BY CASE form from the spec
    let out2 = Command::new("rg")
        .args([
            "-n",
            "ORDER BY CASE.*source",
            root.join("crates").to_str().unwrap(),
            "--glob",
            "!balance.rs",
            "--glob",
            "!balance_helper.rs",
        ])
        .output()
        .unwrap();
    assert!(
        out2.stdout.is_empty(),
        "ORDER BY CASE still duplicated outside balance.rs:\n{}",
        String::from_utf8_lossy(&out2.stdout)
    );
    // Sanity: balance.rs must contain the fragment
    let bal = Command::new("rg")
        .args([
            "-n",
            "CASE.*source.*WHEN.*simplefin",
            root.join("crates/finsight-core/src/repos/balance.rs")
                .to_str()
                .unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        !bal.stdout.is_empty(),
        "balance.rs should contain CASE source fragment, got empty; stdout={:?} stderr={:?} root={:?}",
        String::from_utf8_lossy(&bal.stdout),
        String::from_utf8_lossy(&bal.stderr),
        root
    );
}
