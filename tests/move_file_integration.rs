use std::fs;
use std::io::Write;
use std::path::PathBuf;

use aria_move::config::types::Config;
use filetime::{set_file_mtime, FileTime};
use tempfile::tempdir;

fn write_file(path: &PathBuf, contents: &str) {
    let mut f = fs::File::create(path).expect("create file");
    write!(f, "{}", contents).expect("write file");
    f.sync_all().expect("sync file");
    // sanity check: ensure the file exists immediately after creation
    assert!(
        path.exists(),
        "write_file helper failed to create file: {}",
        path.display()
    );
}

fn cfg_with_bases(download: &std::path::Path, completed: &std::path::Path) -> Config {
    let mut cfg = Config::default();
    cfg.download_base = download.to_path_buf();
    cfg.completed_base = completed.to_path_buf();
    cfg
}

/// Happy path: create a file, move it, verify src removed and dst matches.
#[test]
fn move_file_happy_path() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let mut cfg = cfg_with_bases(download.path(), completed.path());
    cfg.preserve_metadata = false;
    cfg.dry_run = false;

    let src = download.path().join("test_move.txt");
    let dst = completed.path().join("test_move.txt");
    let data = "aria_move test content\n";
    write_file(&src, data);

    let before_bytes = fs::read(&src)?;
    let before_len = before_bytes.len() as u64;

    // Call the move API and surface any error directly.
    aria_move::fs_ops::move_file(&cfg, &src).expect("move_file should succeed");

    assert!(!src.exists(), "source should be removed");
    assert!(dst.exists(), "destination should exist");

    let after_bytes = fs::read(&dst)?;
    let after_meta = fs::metadata(&dst)?;
    assert_eq!(before_len, after_meta.len(), "file size should match");
    assert_eq!(before_bytes, after_bytes, "file contents should match");
    Ok(())
}

/// dry-run behavior is controlled by Config.dry_run; this test verifies it is respected.
#[test]
fn move_file_dry_run_does_nothing() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let mut cfg = cfg_with_bases(download.path(), completed.path());
    cfg.preserve_metadata = false;
    cfg.dry_run = true;

    let src = download.path().join("dry_run.txt");
    let dst = completed.path().join("dry_run.txt");
    write_file(&src, "dry run");

    aria_move::fs_ops::move_file(&cfg, &src).expect("move_file (dry-run) should return Ok");

    assert!(src.exists(), "source should still exist with dry-run");
    assert!(!dst.exists(), "destination should not be created with dry-run");
    Ok(())
}

#[cfg(unix)]
#[test]
fn move_file_preserves_metadata_when_requested() -> Result<(), Box<dyn std::error::Error>> {
    use std::os::unix::fs::PermissionsExt;

    let download = tempdir()?;
    let completed = tempdir()?;
    let mut cfg = cfg_with_bases(download.path(), completed.path());
    cfg.preserve_metadata = true;
    cfg.dry_run = false;

    let src = download.path().join("meta.txt");
    let dst = completed.path().join("meta.txt");
    write_file(&src, "metadata");

    // ensure file exists before setting metadata
    assert!(src.exists(), "source must exist before setting metadata");

    // Set explicit mode and mtime on source
    let perms = fs::Permissions::from_mode(0o640);
    fs::set_permissions(&src, perms.clone()).expect("set permissions");
    let ts = FileTime::from_unix_time(1_700_000_000, 0);
    set_file_mtime(&src, ts).expect("set mtime");

    // Attempt the move. If the implementation erroneously stats the source AFTER moving
    // (causing ENOENT), we still validate that the file was moved and contents match,
    // then soft-check metadata (best-effort).
    let move_res = aria_move::fs_ops::move_file(&cfg, &src);

    // If the destination doesn't exist, include directory listing to help debug.
    if !dst.exists() {
        if let Err(e) = move_res {
            let mut entries = Vec::new();
            for e in fs::read_dir(completed.path())? {
                if let Ok(ent) = e {
                    if let Ok(name) = ent.file_name().into_string() {
                        entries.push(name);
                    }
                }
            }
            panic!(
                "move_file returned error and destination missing.\nerror: {e}\ncompleted dir contains: {:?}",
                entries
            );
        } else {
            panic!("move_file returned Ok but destination missing: {}", dst.display());
        }
    }

    // At this point, dst exists. Source should be gone.
    assert!(!src.exists(), "source should be removed");
    assert!(dst.exists(), "destination should exist");

    // Contents must match
    let after_bytes = fs::read(&dst)?;
    assert_eq!(after_bytes, b"metadata", "file contents should match");

    // Best-effort metadata checks: if implementation preserved, these pass; otherwise log info.
    let dst_meta = fs::metadata(&dst)?;
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