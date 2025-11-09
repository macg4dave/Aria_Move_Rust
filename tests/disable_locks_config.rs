#![cfg(unix)]

use aria_move::{Config, move_entry};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

/// Test that Config.disable_locks=true works to skip directory locking.
/// This verifies the config flag (not just the env var) properly disables locks.
#[test]
fn move_file_succeeds_with_disable_locks_config_flag() {
    let td = tempdir().unwrap();
    let base = td.path();

    let download_base = base.join("incoming");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    // Create source file
    let src = download_base.join("test.txt");
    let mut f = fs::File::create(&src).unwrap();
    writeln!(f, "test data").unwrap();

    // Remove read permission from dest dir (0o333 = write+execute only)
    let mut perms = fs::metadata(&completed_base).unwrap().permissions();
    perms.set_mode(0o333);
    fs::set_permissions(&completed_base, perms).unwrap();

    // Use config flag (not env var) to disable locks
    let mut cfg = Config::default();
    cfg.download_base = PathBuf::from(&download_base);
    cfg.completed_base = PathBuf::from(&completed_base);
    cfg.disable_locks = true; // Set the flag directly

    let dest =
        move_entry(&cfg, &src).expect("move should succeed with disable_locks=true in config");
    assert!(dest.exists(), "destination file should exist");

    // Cleanup: restore perms for tempdir deletion
    let mut restore = fs::metadata(&completed_base).unwrap().permissions();
    restore.set_mode(0o755);
    let _ = fs::set_permissions(&completed_base, restore);
}
