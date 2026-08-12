use std::{env, fs, path::PathBuf};

fn ensure_sidecar_placeholder() {
    let Ok(target) = env::var("TARGET") else {
        return;
    };
    let suffix = if target.contains("windows") { ".exe" } else { "" };
    let path = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap_or_default())
        .join("binaries")
        .join(format!("funo-{target}{suffix}"));
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("failed to create the sidecar staging directory");
    }
    // `cargo test` still evaluates Tauri's bundle configuration even though it
    // never packages or starts the CLI sidecar. The release preparation script
    // replaces this marker with the real, target-specific executable before a
    // Tauri bundle is built.
    fs::write(path, []).expect("failed to create the test sidecar marker");
}

fn main() {
    ensure_sidecar_placeholder();
    tauri_build::build()
}
