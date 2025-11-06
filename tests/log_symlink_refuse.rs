#![cfg(unix)]

//! Ensures path_has_symlink_ancestor catches a symlinked parent of the log file directory.
//! Runs the check in a child process with HOME pointing to a temp directory, so it never
//! touches the real user's ~/Library/... and cleans up after the test.

use std::fs;
use std::os::unix::fs as unix_fs;
use std::process::Command;
use tempfile::tempdir;
use std::path::PathBuf;

#[test]
fn log_file_symlink_parent_refused() {
    // Parent sets up a temp HOME for the child and a separate "outside" dir.
    let temp_home = tempdir().expect("temp HOME");
    let outside = tempdir().expect("outside target");

    // Spawn the current test binary, running only the ignored child test with HOME set.
    let me = std::env::current_exe().expect("current test binary");
    let status = Command::new(me)
        .arg("--ignored")
        .arg("log_file_symlink_parent_refused_child")
        .env("HOME", temp_home.path())        // redirect all HOME-based paths into temp space
        .env("ARIA_MOVE_OUTSIDE", outside.path())
        .status()
        .expect("spawn child test");
    assert!(status.success(), "child test failed with status: {status}");
    // temp_home and outside are cleaned up automatically here
}

#[test]
#[ignore]
fn log_file_symlink_parent_refused_child() {
    // Compute the default log path using HOME from the environment (temp HOME from parent).
    let log_path = aria_move::config::paths::default_log_path().expect("default_log_path present");

    // Replace its directory (<...>/aria_move) with a symlink to OUTSIDE, all under temp HOME.
    let aria_dir = log_path.parent().expect("log_path has parent").to_path_buf();
    let outside = std::path::PathBuf::from(
        std::env::var_os("ARIA_MOVE_OUTSIDE").expect("OUTSIDE provided by parent"),
    );

    // Ensure the parent of aria_dir exists (e.g., $HOME/Library/Application Support).
    let parent = aria_dir.parent().expect("aria_dir has parent");
    fs::create_dir_all(parent).unwrap();

    // Remove the real directory if present, then create the symlink in its place.
    if aria_dir.exists() {
        fs::remove_dir_all(&aria_dir).unwrap();
    }
    unix_fs::symlink(&outside, &aria_dir).unwrap();

    // Now the computed log_path should have a symlinked ancestor.
    // call the public helper from the crate root (ensure correct path)
    assert!(
        aria_move::path_has_symlink_ancestor(&log_path).unwrap(),
        "symlink ancestor should be detected for {}",
        log_path.display()
    );
}
