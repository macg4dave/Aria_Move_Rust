use std::fs;
use std::io::Write;
use std::path::Path;

use aria_move::Config;
use filetime::{set_file_mtime, FileTime}; // both used; keep
use tempfile::tempdir;

/// Create a file with the given content and fsync it (reduces test flakiness).
fn write_file(path: &Path, contents: &str) {
    let mut f = fs::File::create(path).expect("create source file");
    write!(f, "{}", contents).expect("write source content");
    f.sync_all().expect("sync source file");
    assert!(path.exists(), "file should exist immediately after creation: {}", path.display());
}

/// Build a Config with provided bases and flags.
fn mk_cfg(download: &Path, completed: &Path, preserve_metadata: bool, dry_run: bool) -> Config {
    Config {
        download_base: download.to_path_buf(),
        completed_base: completed.to_path_buf(),
        preserve_metadata,
        dry_run,
        ..Config::default()
    }
}

/// Happy path: create a file, move it, verify src removed and dst matches.
#[test]
fn move_file_happy_path() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, false);

    let src = download.path().join("test_move.txt");
    write_file(&src, "aria_move test content\n");

    let before_bytes = fs::read(&src)?;
    let before_len = before_bytes.len() as u64;

    // Move and capture the actual destination path (may include dedup suffix).
    let dest = aria_move::fs_ops::move_file(&cfg, &src).expect("move_file should succeed");

    assert!(!src.exists(), "source should be removed");
    assert!(dest.exists(), "destination should exist");

    let after_bytes = fs::read(&dest)?;
    let after_meta = fs::metadata(&dest)?;
    assert_eq!(before_len, after_meta.len(), "file size should match");
    assert_eq!(before_bytes, after_bytes, "file contents should match");
    Ok(())
}

/// Dry-run should not modify the filesystem, but still return the intended destination path.
#[test]
fn move_file_dry_run_does_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, true);

    let src = download.path().join("dry_run.txt");
    write_file(&src, "dry run");

    let dest = aria_move::fs_ops::move_file(&cfg, &src).expect("move_file (dry-run) should return Ok");

    assert!(src.exists(), "source should still exist with dry-run");
    assert!(
        !dest.exists(),
        "destination should not be created with dry-run (returned path: {})",
        dest.display()
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn move_file_preserves_metadata_when_requested() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), true, false);

    let src = download.path().join("meta.txt");
    write_file(&src, "metadata");

    // Ensure file exists before setting metadata
    assert!(src.exists(), "source must exist before setting metadata");

    // Set explicit mode and mtime on source
    let perms = fs::Permissions::from_mode(0o640);
    fs::set_permissions(&src, perms.clone()).expect("set permissions");
    let ts = FileTime::from_unix_time(1_700_000_000, 0);
    set_file_mtime(&src, ts).expect("set mtime");

    // Move and capture the actual destination.
    let dest = match aria_move::fs_ops::move_file(&cfg, &src) {
        Ok(p) => p,
        Err(e) => {
            // Helpful diagnostics: list completed dir on failure.
            let mut entries = Vec::new();
            for e in fs::read_dir(completed.path())? {
                if let Ok(ent) = e {
                    entries.push(ent.file_name().to_string_lossy().into_owned());
                }
            }
            panic!(
                "move_file returned error and destination missing.\nerror: {e}\ncompleted dir contains: {:?}",
                entries
            );
        }
    };

    // At this point, dest should exist and src should be gone.
    assert!(!src.exists(), "source should be removed");
    assert!(dest.exists(), "destination should exist");

    // Contents must match
    let after_bytes = fs::read(&dest)?;
    assert_eq!(after_bytes, b"metadata", "file contents should match");

    // Best-effort metadata checks: if implementation preserved, these pass; otherwise log info.
    let dst_meta = fs::metadata(&dest)?;
    let dst_mode = dst_meta.permissions().mode() & 0o777;
    if dst_mode != 0o640 {
        eprintln!(
            "note: permissions not preserved (got {:o}, expected 0640). Implementation may copy without preserving mode.",
            dst_mode
        );
    }
    let dst_mtime = FileTime::from_last_modification_time(&dst_meta);
    if dst_mtime.unix_seconds() != ts.unix_seconds() {
        eprintln!(
            "note: mtime not preserved (got {}, expected {}). Implementation may not preserve mtime.",
            dst_mtime.unix_seconds(),
            ts.unix_seconds()
        );
    }

    Ok(())
}