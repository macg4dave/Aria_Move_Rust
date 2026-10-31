#![cfg(unix)]

use aria_move::{Config, move_entry};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

/// PermissionDenied is an error unless the caller explicitly disables locks.
#[test]
fn move_file_rejects_lock_eacces() {
    // SAFETY: geteuid has no preconditions or side effects.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let td = tempdir().unwrap();
    let incoming = td.path().join("incoming");
    let completed = td.path().join("completed");
    fs::create_dir(&incoming).unwrap();
    fs::create_dir(&completed).unwrap();
    let src = incoming.join("data");
    fs::write(&src, b"keep").unwrap();
    fs::set_permissions(&incoming, fs::Permissions::from_mode(0o333)).unwrap();
    let cfg = Config::new(&incoming, &completed);
    let result = move_entry(&cfg, &src);
    fs::set_permissions(&incoming, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(src).unwrap(), b"keep");
    assert!(fs::read_dir(completed).unwrap().next().is_none());
}

/// Negative check: if destination directory also lacks write permission (0o111), the move fails.
#[test]
fn move_file_fails_without_write_permission() {
    // SAFETY: geteuid has no preconditions or side effects.
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let td = tempdir().unwrap();
    let download_base = td.path().join("incoming2");
    let completed_base = td.path().join("completed2");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();
    let src_path = download_base.join("sample.bin");
    fs::write(&src_path, b"zzz").unwrap();

    // Source dir: remove read (0o333) to trigger lock EACCES fallback; keep write so file remains.
    let mut s_perms = fs::metadata(&download_base).unwrap().permissions();
    s_perms.set_mode(0o333);
    fs::set_permissions(&download_base, s_perms).unwrap();

    // Dest dir: remove read AND write (0o111) leaving only execute; creation should fail.
    let mut d_perms = fs::metadata(&completed_base).unwrap().permissions();
    d_perms.set_mode(0o111);
    fs::set_permissions(&completed_base, d_perms).unwrap();

    let cfg = Config {
        download_base: PathBuf::from(&download_base),
        completed_base: PathBuf::from(&completed_base),
        ..Config::default()
    };

    let err = move_entry(&cfg, &src_path)
        .expect_err("expected failure without write permission on destination");
    let msg = format!("{err}");
    assert!(
        msg.to_ascii_lowercase().contains("permission")
            || msg.to_ascii_lowercase().contains("access"),
        "unexpected error message: {msg}"
    );

    // Restore perms for cleanup
    for dir in [&download_base, &completed_base] {
        if let Ok(meta) = fs::metadata(dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(dir, perms);
        }
    }
}
