#![cfg(target_os = "linux")]

use aria_move::{move_entry, Config};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

/// Ensure we surface a clear permission denied error when the destination directory is not writable.
/// This reproduces Debian/Ubuntu "Permission denied (os error 13)" conditions.
#[test]
fn move_file_into_nonwritable_dest_yields_eacces() {
    // Skip if running as root; root may bypass permission checks and the test won't behave as expected.
    unsafe {
        if libc::geteuid() == 0 {
            eprintln!("skipping: running as root");
            return;
        }
    }

    let td = tempdir().expect("tempdir");
    let base = td.path();

    // Prepare download_base with a sample file
    let download_base = base.join("incoming");
    fs::create_dir_all(&download_base).unwrap();
    let src_file = download_base.join("sample.txt");
    let mut f = fs::File::create(&src_file).unwrap();
    writeln!(f, "hello").unwrap();

    // Prepare completed_base and make it non-writable (0555)
    let completed_base = base.join("completed");
    fs::create_dir_all(&completed_base).unwrap();
    let mut perms = fs::metadata(&completed_base).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&completed_base, perms).unwrap();

    // Build config pointing to these bases
    let mut cfg = Config::default();
    cfg.download_base = PathBuf::from(&download_base);
    cfg.completed_base = PathBuf::from(&completed_base);

    // Attempt the move (should fail with EACCES/permission denied)
    let err = move_entry(&cfg, &src_file).expect_err("expected permission denied error");
    let msg = format!("{}", err);

    // Our helper adds a hint and OS code; assert key parts for Debian/Linux
    assert!(
        msg.to_ascii_lowercase().contains("permission denied") ||
        msg.contains("[os code: 13]") ||
        msg.to_ascii_lowercase().contains("read-only filesystem")
    , "unexpected error: {}", msg);

    // Restore permissions so tempdir cleanup can remove the directory on all platforms
    let mut restore = fs::metadata(&completed_base).unwrap().permissions();
    restore.set_mode(0o755);
    let _ = fs::set_permissions(&completed_base, restore);
}
