#![cfg(unix)]

use aria_move::{move_entry, Config};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

/// With ARIA_MOVE_DISABLE_LOCKS=1, moving a directory should succeed even when
/// the destination directory denies read permission, provided write+execute are allowed.
#[test]
fn move_dir_succeeds_when_dest_no_read_with_locks_disabled() {
    let td = tempdir().unwrap();
    let base = td.path();

    let download_base = base.join("incoming");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    // Create a small source directory with a file
    let src_dir = download_base.join("folder");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("a.txt"), b"hello").unwrap();

    // Remove read permission from dest dir while keeping write+execute (0o333)
    let mut perms = fs::metadata(&completed_base).unwrap().permissions();
    perms.set_mode(0o333);
    fs::set_permissions(&completed_base, perms).unwrap();

    // Enable lock skipping
    unsafe {
        std::env::set_var("ARIA_MOVE_DISABLE_LOCKS", "1");
    }

    let mut cfg = Config::default();
    cfg.download_base = PathBuf::from(&download_base);
    cfg.completed_base = PathBuf::from(&completed_base);

    let dest = move_entry(&cfg, &src_dir).expect("dir move should succeed without directory read perms when locks disabled");
    assert!(dest.exists(), "destination directory should exist");
    assert!(dest.join("a.txt").exists(), "file should be present in destination");

    // Restore perms to allow cleanup
    let mut restore = fs::metadata(&completed_base).unwrap().permissions();
    restore.set_mode(0o755);
    let _ = fs::set_permissions(&completed_base, restore);

    unsafe {
        std::env::remove_var("ARIA_MOVE_DISABLE_LOCKS");
    }
}
