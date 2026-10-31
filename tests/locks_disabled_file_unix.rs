#![cfg(unix)]

use aria_move::{Config, move_entry};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

/// With Config.disable_locks=true, moving a file should succeed even when the
/// destination directory denies read permission (no ability to open for O_RDONLY),
/// as long as write+execute are allowed for creation and rename.
#[test]
fn move_file_succeeds_when_dest_no_read_with_locks_disabled() {
    let td = tempdir().unwrap();
    let base = td.path();

    let download_base = base.join("incoming");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    // Create source file
    let src = download_base.join("f.bin");
    let mut f = fs::File::create(&src).unwrap();
    writeln!(f, "data").unwrap();

    // Remove read permission from dest dir while keeping write+execute (0o333)
    // Owner is current user (test runner), so write/exec suffice to create/rename entries.
    let mut perms = fs::metadata(&completed_base).unwrap().permissions();
    perms.set_mode(0o333);
    fs::set_permissions(&completed_base, perms).unwrap();

    // Disable locking explicitly through configuration

    let cfg = Config {
        disable_locks: true,
        download_base: PathBuf::from(&download_base),
        completed_base: PathBuf::from(&completed_base),
        ..Config::default()
    };

    let dest = move_entry(&cfg, &src)
        .expect("move should succeed without directory read perms when locks disabled");
    assert!(dest.exists(), "destination should exist");

    // Cleanup: restore perms so tempdir can cleanly delete on all systems
    let mut restore = fs::metadata(&completed_base).unwrap().permissions();
    restore.set_mode(0o755);
    let _ = fs::set_permissions(&completed_base, restore);
}
