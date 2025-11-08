use aria_move::{Config, fs_ops};
use std::fs;
use std::io::Write;
use tempfile::tempdir;

fn mk_cfg(
    download: &std::path::Path,
    completed: &std::path::Path,
    preserve_metadata: bool,
    dry_run: bool,
) -> Config {
    Config {
        download_base: download.to_path_buf(),
        completed_base: completed.to_path_buf(),
        preserve_metadata,
        dry_run,
        ..Config::default()
    }
}

#[test]
fn move_dir_atomic_or_copy_ok() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), true, false);

    // Build source directory tree
    let src_dir = download.path().join("project");
    fs::create_dir_all(&src_dir)?;
    let sub = src_dir.join("sub");
    fs::create_dir_all(&sub)?;
    let file_a = src_dir.join("a.txt");
    let file_b = sub.join("b.log");
    {
        let mut fa = fs::File::create(&file_a)?;
        writeln!(fa, "alpha")?;
        let mut fb = fs::File::create(&file_b)?;
        writeln!(fb, "beta")?;
    }

    // Capture metadata for later best-effort verification
    let meta_a = fs::metadata(&file_a)?;
    let meta_b = fs::metadata(&file_b)?;

    let dest = fs_ops::move_dir(&cfg, &src_dir)?;
    assert!(!src_dir.exists(), "source directory should be removed");
    assert!(dest.exists(), "destination directory should exist");

    let moved_a = dest.join("a.txt");
    let moved_b = dest.join("sub/b.log");
    assert!(moved_a.exists());
    assert!(moved_b.exists());
    assert_eq!(fs::read_to_string(moved_a)?, "alpha\n");
    assert_eq!(fs::read_to_string(moved_b)?, "beta\n");

    // Best-effort metadata: size match is required; mode/time may differ; we just assert lengths.
    assert_eq!(meta_a.len(), fs::metadata(dest.join("a.txt"))?.len());
    assert_eq!(meta_b.len(), fs::metadata(dest.join("sub/b.log"))?.len());
    Ok(())
}

#[test]
fn move_dir_dry_run() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, true);

    let src_dir = download.path().join("dry_project");
    fs::create_dir_all(&src_dir)?;
    let file = src_dir.join("x.txt");
    fs::write(&file, "dry")?;

    let dest = fs_ops::move_dir(&cfg, &src_dir)?;
    assert!(src_dir.exists(), "source should remain on dry-run");
    assert!(
        !dest.exists(),
        "destination should not be created on dry-run"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn move_dir_partial_failure_cleanup() -> Result<(), Box<dyn std::error::Error>> {
    // Force copy path to exercise cleanup logic
    unsafe {
        std::env::set_var("ARIA_MOVE_FORCE_DIR_COPY", "1");
    }

    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, false);

    // Build a simple tree
    let src_dir = download.path().join("failtree");
    fs::create_dir_all(&src_dir)?;
    let f = src_dir.join("x.txt");
    fs::write(&f, "x")?;

    // Make completed base read-only so creating target or files fails
    use std::os::unix::fs::PermissionsExt;
    let mut base_perms = fs::metadata(completed.path())?.permissions();
    base_perms.set_mode(0o555); // read/exec only
    fs::set_permissions(completed.path(), base_perms)?;

    let result = fs_ops::move_dir(&cfg, &src_dir);
    assert!(
        result.is_err(),
        "expected error due to unwritable destination"
    );

    // Target directory should not persist after failure
    let target = completed.path().join("failtree");
    assert!(
        !target.exists(),
        "partial target tree should be cleaned up on failure"
    );

    // Unset the env var
    unsafe {
        std::env::remove_var("ARIA_MOVE_FORCE_DIR_COPY");
    }
    Ok(())
}
