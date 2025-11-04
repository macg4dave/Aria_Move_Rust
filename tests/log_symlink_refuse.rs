#![cfg(unix)]

use std::fs;
use std::os::unix::fs as unix_fs;
use tempfile::tempdir;
use std::env;

#[test]
fn log_file_symlink_parent_refused() {
    // Set XDG_DATA_HOME so default_log_path resolves to a temp location
    let data = tempdir().unwrap();
    let outside = tempdir().unwrap();
    env::set_var("XDG_DATA_HOME", data.path());

    // create a symlink at $XDG_DATA_HOME/aria_move -> outside
    let aria_dir = data.path().join("aria_move");
    let parent = aria_dir.parent().unwrap();
    fs::create_dir_all(parent).unwrap();
    unix_fs::symlink(outside.path(), &aria_dir).unwrap();

    // compute default log path and assert path_has_symlink_ancestor detects the symlink
    let log_path = aria_move::default_log_path().expect("default_log_path present");
    assert!(aria_move::path_has_symlink_ancestor(&log_path).unwrap(), "symlink ancestor should be detected");
}