#![cfg(unix)]

use aria_move::{move_entry, Config};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use tempfile::tempdir;

/// Unit-style check: acquiring a directory lock on a directory without read permission
/// should yield PermissionDenied (EACCES). We don't call `acquire_dir_lock` directly because
/// it's not public; instead we simulate via a move operation, which internally attempts
/// to lock source and destination directories. The code now falls back when EACCES occurs.
#[test]
fn move_file_fallbacks_when_lock_eacces() {
    let td = tempdir().unwrap();
    let download_base = td.path().join("incoming");
    let completed_base = td.path().join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    // Create a source file
    let src_path = download_base.join("sample.dat");
    fs::write(&src_path, b"abc123").unwrap();

    // Remove read permission from both directories (0o333 = write+exec only for owner)
    for dir in [&download_base, &completed_base] {
        let mut perms = fs::metadata(dir).unwrap().permissions();
        perms.set_mode(0o333); // remove read bits, keep write/exec for owner
        fs::set_permissions(dir, perms).unwrap();
    }

    // Build config
    let mut cfg = Config::default();
    cfg.download_base = PathBuf::from(&download_base);
    cfg.completed_base = PathBuf::from(&completed_base);

    // Perform the move: should succeed because fallback skips locks on EACCES
    let dest = move_entry(&cfg, &src_path).expect("move should succeed with lock EACCES fallback");
    assert!(dest.exists(), "destination file should exist");

    // Restore permissions to allow cleanup.
    for dir in [&download_base, &completed_base] {
        if let Ok(meta) = fs::metadata(dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(dir, perms);
        }
    }
}

/// Negative check: if destination directory also lacks write permission (0o111), the move fails.
#[test]
fn move_file_fails_without_write_permission() {
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

    let mut cfg = Config::default();
    cfg.download_base = PathBuf::from(&download_base);
    cfg.completed_base = PathBuf::from(&completed_base);

    let err = move_entry(&cfg, &src_path).expect_err("expected failure without write permission on destination");
    let msg = format!("{err}");
    assert!(msg.to_ascii_lowercase().contains("permission") || msg.to_ascii_lowercase().contains("access"), "unexpected error message: {msg}");

    // Restore perms for cleanup
    for dir in [&download_base, &completed_base] {
        if let Ok(meta) = fs::metadata(dir) {
            let mut perms = meta.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(dir, perms);
        }
    }
}
