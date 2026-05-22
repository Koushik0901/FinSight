// refinery uses `embed_migrations!` at runtime; build.rs only re-runs Cargo
// when migrations change.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
