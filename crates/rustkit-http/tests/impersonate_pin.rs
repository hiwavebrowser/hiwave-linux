//! Z2-T1 pin: the Chrome-shaped test handshake must stay unreachable from the
//! shipped browser. `impersonate-test` is off by default, and no manifest in
//! the workspace other than rustkit-http's own may name it.

use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

#[test]
fn default_build_has_no_chrome_handshake() {
    #[cfg(not(feature = "impersonate-test"))]
    assert!(!cfg!(feature = "impersonate-test"));
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    let default_line = manifest.lines().find(|l| l.trim_start().starts_with("default")).unwrap();
    assert!(!default_line.contains("impersonate"), "default features must not enable impersonate-test");
}

#[test]
fn no_other_manifest_enables_impersonate_test() {
    let root = workspace_root();
    let mut manifests = vec![root.join("Cargo.toml")];
    for entry in fs::read_dir(root.join("crates")).unwrap() {
        let path = entry.unwrap().path().join("Cargo.toml");
        if path.exists() && !path.starts_with(root.join("crates/rustkit-http")) {
            manifests.push(path);
        }
    }
    let mut checked = 0;
    for m in manifests {
        let text = fs::read_to_string(&m).unwrap();
        assert!(
            !text.contains("impersonate-test"),
            "{} must not mention impersonate-test (harness-only profile)",
            m.display()
        );
        checked += 1;
    }
    assert!(checked > 5, "pin walked too few manifests ({checked}); layout changed?");
}

#[test]
fn hiwave_app_does_not_depend_on_boring() {
    let app = workspace_root().join("crates/hiwave-app/Cargo.toml");
    let text = fs::read_to_string(app).unwrap();
    assert!(!text.contains("boring"), "hiwave-app must not depend on boring");
}
