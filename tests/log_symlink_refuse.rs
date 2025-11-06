#![cfg(unix)]

//! Ensures path_has_symlink_ancestor flags a symlinked parent of the log file directory.
//!
//! Strategy:
//! - Parent test runs a child process with HOME pointed to a temp dir (isolation).
//! - Child computes the default log path, replaces its parent dir with a symlink,
//!   and asserts the helper detects the symlink ancestor.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn log_file_symlink_parent_refused() {
    // Parent sets up isolated HOME and a separate "outside" dir.
    let temp_home = tempdir().expect("temp HOME");
    let outside = tempdir().expect("outside target");

    // Spawn the current test binary to run only the ignored child test under the temp HOME.
    let me = std::env::current_exe().expect("current test binary");
    let status = Command::new(me)
        .arg("--ignored")
        .arg("log_file_symlink_parent_refused_child")
        .env("HOME", temp_home.path()) // all HOME-based paths now point into temp
        .env("ARIA_MOVE_OUTSIDE", outside.path())
        .status()
        .expect("spawn child test");
    assert!(status.success(), "child test failed: {status}");
}

#[test]
#[ignore]
fn log_file_symlink_parent_refused_child() {
    // Compute the default log path using HOME from the environment (temp HOME from parent).
    let log_path =
        aria_move::config::paths::default_log_path().expect("default_log_path should resolve");

    // Baseline: should not report a symlink ancestor yet.
    assert!(
        !aria_move::path_has_symlink_ancestor(&log_path).unwrap(),
        "baseline: no symlink ancestor expected for {}",
        log_path.display()
    );

    // Derive the aria_move directory and its parent (e.g., ~/.local/share/aria_move on Linux).
    let aria_dir = log_path.parent().expect("log_path has parent").to_path_buf();
    let parent_of_aria = aria_dir.parent().expect("aria_dir has parent");

    // Create the parent path (e.g., ~/.local/share or ~/Library/Application Support).
    fs::create_dir_all(parent_of_aria).unwrap();

    // Replace aria_dir with a symlink to OUTSIDE (still all within our temp HOME).
    let outside = std::path::PathBuf::from(
        std::env::var_os("ARIA_MOVE_OUTSIDE").expect("OUTSIDE provided by parent"),
    );
    if aria_dir.exists() {
        fs::remove_dir_all(&aria_dir).unwrap();
    }
    unix_fs::symlink(&outside, &aria_dir).unwrap();

    // Now the computed log_path should have a symlinked ancestor.
    assert!(
        aria_move::path_has_symlink_ancestor(&log_path).unwrap(),
        "symlink ancestor should be detected for {}",
        log_path.display()
    );
}
