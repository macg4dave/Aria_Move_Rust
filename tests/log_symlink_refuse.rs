#![cfg(unix)]

//! Ensures path_has_symlink_ancestor catches a symlinked parent of the log file directory.
//! Spawns a child test, computes the real default_log_path, then swaps its dir with a symlink.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn log_file_symlink_parent_refused() {
    // Parent creates a target dir to point the symlink at and spawns a child.
    let outside = tempdir().unwrap();

    let me = std::env::current_exe().expect("current test binary");
    let status = Command::new(me)
        .arg("--ignored")
        .arg("log_file_symlink_parent_refused_child")
        .env("ARIA_MOVE_OUTSIDE", outside.path())
        .status()
        .expect("spawn child test");
    assert!(status.success(), "child test failed with status: {status}");
}

#[test]
#[ignore]
fn log_file_symlink_parent_refused_child() {
    // Compute the default log path using the current environment.
    let log_path = aria_move::default_log_path().expect("default_log_path present");

    // Replace its directory (<...>/aria_move) with a symlink to OUTSIDE.
    let aria_dir = log_path.parent().expect("log_path has parent");
    let outside = std::env::var_os("ARIA_MOVE_OUTSIDE").expect("OUTSIDE provided by parent");

    // Remove the real directory if it was created by default_log_path.
    if aria_dir.exists() {
        fs::remove_dir_all(aria_dir).unwrap();
    }
    // Ensure the parent of aria_dir exists.
    let parent = aria_dir.parent().expect("aria_dir has parent");
    fs::create_dir_all(parent).unwrap();

    // Create the symlink in place of the directory.
    unix_fs::symlink(&outside, aria_dir).unwrap();

    // Now the computed log_path should have a symlinked ancestor.
    assert!(
        aria_move::path_has_symlink_ancestor(&log_path).unwrap(),
        "symlink ancestor should be detected for {}",
        log_path.display()
    );
}
