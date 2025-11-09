#![cfg(unix)]

use aria_move::{move_entry, Config};
use std::fs;
use tempfile::tempdir;

/// Even when running as root, moving the download base itself is rejected.
#[test]
fn root_cannot_move_download_base_itself() {
    unsafe {
        if libc::geteuid() != 0 {
            eprintln!("skipping: not running as root");
            return;
        }
    }

    let td = tempdir().unwrap();
    let download_base = td.path().join("incoming");
    let completed_base = td.path().join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    let mut cfg = Config::default();
    cfg.download_base = download_base.clone();
    cfg.completed_base = completed_base;

    // Try to move the base directory itself (should be refused)
    let err = move_entry(&cfg, &download_base).expect_err("expected refusal moving base directory");
    let msg = format!("{err}");
    assert!(msg.to_ascii_lowercase().contains("refusing") || msg.to_ascii_lowercase().contains("download base"), "unexpected error: {msg}");
}
