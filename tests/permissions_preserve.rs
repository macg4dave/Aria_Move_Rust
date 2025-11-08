#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

/// Verify that --preserve-permissions (config flag) preserves mode bits on file move.
#[test]
fn file_move_preserve_permissions_only() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let mut cfg = aria_move::Config::default();
    cfg.download_base = download.path().to_path_buf();
    cfg.completed_base = completed.path().to_path_buf();
    cfg.preserve_metadata = false;
    cfg.preserve_permissions = true;

    let src = download.path().join("perm.txt");
    fs::write(&src, b"perm")?;

    // Set explicit mode on source
    let desired_mode = 0o640;
    fs::set_permissions(&src, fs::Permissions::from_mode(desired_mode))?;

    let dest = aria_move::fs_ops::move_file(&cfg, &src)?;
    assert!(!src.exists(), "source removed");
    assert!(dest.exists(), "dest exists");

    let dst_mode = fs::metadata(&dest)?.permissions().mode() & 0o777;
    assert_eq!(dst_mode, desired_mode, "destination mode should match source");
    Ok(())
}

/// Verify permissions preservation during directory moves with the flag.
#[test]
fn dir_move_preserve_permissions_only() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let mut cfg = aria_move::Config::default();
    cfg.download_base = download.path().to_path_buf();
    cfg.completed_base = completed.path().to_path_buf();
    cfg.preserve_metadata = false;
    cfg.preserve_permissions = true;

    let dir = download.path().join("d");
    fs::create_dir_all(&dir)?;
    let a = dir.join("a.bin");
    let b = dir.join("b.bin");
    fs::write(&a, b"a")?;
    fs::write(&b, b"b")?;

    // Set different modes
    fs::set_permissions(&a, fs::Permissions::from_mode(0o600))?;
    fs::set_permissions(&b, fs::Permissions::from_mode(0o644))?;

    let dest_dir = aria_move::fs_ops::move_dir(&cfg, &dir)?;
    assert!(!dir.exists(), "source dir removed");
    assert!(dest_dir.exists(), "dest dir exists");

    let da = dest_dir.join("a.bin");
    let db = dest_dir.join("b.bin");
    let ma = fs::metadata(&da)?.permissions().mode() & 0o777;
    let mb = fs::metadata(&db)?.permissions().mode() & 0o777;
    assert_eq!(ma, 0o600, "a.bin mode should be preserved");
    assert_eq!(mb, 0o644, "b.bin mode should be preserved");
    Ok(())
}
