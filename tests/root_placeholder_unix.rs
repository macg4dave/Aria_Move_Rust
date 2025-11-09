#![cfg(unix)]

use aria_move::config::validate_and_normalize;
use aria_move::Config;
use std::path::PathBuf;

/// On Unix (Linux/macOS), when running as root, we refuse to create the
/// template placeholder paths from the default config for safety.
#[test]
fn root_rejects_placeholder_paths() {
    // Only meaningful when run as root
    unsafe {
        if libc::geteuid() != 0 {
            eprintln!("skipping: not running as root");
            return;
        }
    }

    let mut cfg = Config::default();
    // Simulate an unedited default config
    cfg.download_base = PathBuf::from("/path/to/incoming");
    cfg.completed_base = PathBuf::from("/path/to/completed");

    let err = validate_and_normalize(&mut cfg).expect_err("expected refusal for placeholder paths as root");
    let msg = format!("{err}");
    assert!(msg.contains("Refusing to create placeholder default path") || msg.contains("placeholder"), "unexpected error: {msg}");
}
