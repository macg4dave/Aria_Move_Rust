#![cfg(unix)]

//! Ensures path_has_symlink_ancestor flags a symlinked parent of a target path.
//!
//! On macOS, /var is a symlink to /private/var, which can make a naive baseline
//! check fail. To avoid ambient symlinks, we canonicalize the temp base first and
//! operate entirely under that canonicalized path.

use std::fs;
use std::os::unix::fs as unix_fs;
use tempfile::tempdir;

#[test]
fn detects_symlinked_parent() {
    // Base temp dir; canonicalize to avoid ambient symlinks (/var -> /private/var on macOS).
    let td = tempdir().expect("tempdir");
    let base = fs::canonicalize(td.path()).expect("canonicalize tempdir");

    // Construct a log-like path we control: <base>/no_symlink_parent/aria_move/aria_move.log
    let parent_plain = base.join("no_symlink_parent");
    let aria_dir = parent_plain.join("aria_move");
    let log_path = aria_dir.join("aria_move.log");

    // Ensure baseline directories exist without symlinks.
    fs::create_dir_all(&aria_dir).expect("create aria_dir");

    // Baseline: no symlink ancestors expected now.
    let baseline = aria_move::path_has_symlink_ancestor(&log_path).unwrap_or(false);
    assert!(
        !baseline,
        "baseline should have no symlink ancestors: {}",
        log_path.display()
    );

    // Create an "outside" real directory to point the symlink to.
    let outside = base.join("outside_real_dir");
    fs::create_dir_all(&outside).expect("create outside");

    // Replace aria_dir with a symlink to outside.
    fs::remove_dir_all(&aria_dir).expect("remove aria_dir");
    unix_fs::symlink(&outside, &aria_dir).expect("symlink aria_dir -> outside");

    // Now detection should see a symlink ancestor.
    let detected = aria_move::path_has_symlink_ancestor(&log_path).unwrap_or(false);
    assert!(
        detected,
        "symlink ancestor should be detected for {} (aria_dir={}, outside={})",
        log_path.display(),
        aria_dir.display(),
        outside.display()
    );
}
