use std::fs;
use std::io::Write;
use std::path::Path;

use aria_move::Config;
use tempfile::tempdir;

/// Create a file with the given content and fsync it (reduces test flakiness).
fn write_file(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).expect("create parent dirs"); }
    let mut f = fs::File::create(path).expect("create file");
    write!(f, "{}", contents).expect("write content");
    f.sync_all().expect("sync file");
    assert!(path.exists(), "file should exist immediately: {}", path.display());
}

/// Build a Config with provided bases and flags.
fn mk_cfg(download: &Path, completed: &Path, preserve_metadata: bool, dry_run: bool) -> Config {
    let mut cfg = Config::default();
    cfg.download_base = download.to_path_buf();
    cfg.completed_base = completed.to_path_buf();
    cfg.preserve_metadata = preserve_metadata;
    cfg.dry_run = dry_run;
    cfg
}

/// Move a directory with nested files; verify contents preserved and source removed.
#[test]
fn move_dir_happy_path_nested() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, false);

    // Create nested directory tree with files
    let src_dir = download.path().join("album");
    let f1 = src_dir.join("track1.flac");
    let f2 = src_dir.join("disc2").join("track2.flac");
    write_file(&f1, "one");
    write_file(&f2, "two");

    // Pre-read contents to compare after move
    let before1 = fs::read(&f1)?;
    let before2 = fs::read(&f2)?;

    // Perform directory move
    let dest = aria_move::fs_ops::move_dir(&cfg, &src_dir).expect("move_dir should succeed");

    // Source dir removed; destination directory exists
    assert!(!src_dir.exists(), "source directory should be removed");
    assert!(dest.exists(), "destination directory should exist: {}", dest.display());

    // Files should exist under destination with same relative layout and contents
    let d1 = dest.join("track1.flac");
    let d2 = dest.join("disc2").join("track2.flac");
    assert!(d1.exists(), "dest file missing: {}", d1.display());
    assert!(d2.exists(), "dest file missing: {}", d2.display());
    assert_eq!(before1, fs::read(&d1)?);
    assert_eq!(before2, fs::read(&d2)?);

    Ok(())
}

/// Dry-run for directories should not modify the filesystem, but return intended destination path.
#[test]
fn move_dir_dry_run_does_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, true);

    let src_dir = download.path().join("photos");
    let f = src_dir.join("img.jpg");
    write_file(&f, "jpgdata");

    let dest = aria_move::fs_ops::move_dir(&cfg, &src_dir).expect("move_dir dry-run should return Ok");

    // With dry-run, no changes on disk
    assert!(src_dir.exists(), "source directory should still exist");
    assert!(f.exists(), "source file should still exist");
    assert!(
        !dest.exists(),
        "destination should not be created with dry-run (returned path: {})",
        dest.display()
    );
    Ok(())
}
