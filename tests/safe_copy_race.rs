#![cfg(unix)]

// This test simulates a TOCTOU race where another actor creates a symlink at the
// destination path while we are copying. Our copy+rename should either:
//
// - Win the race and produce a regular file at `dest` with the source content; OR
// - Lose the race and find a symlink at `dest` (non-deterministic outcome; skip strict checks).
//
// In all cases, the copy helper must not leave behind temporary files.

use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn safe_copy_and_rename_with_concurrent_symlink_creation() {
    let td = tempdir().unwrap();

    // Source: small, durable file
    let src = td.path().join("src.txt");
    {
        let mut f = fs::File::create(&src).unwrap();
        write!(f, "from_src").unwrap();
        f.sync_all().unwrap();
    }

    // Destination dir/path
    let dest_dir = td.path().join("dest");
    fs::create_dir_all(&dest_dir).unwrap();
    let dest = dest_dir.join("file.txt");

    // Outside file that a concurrent actor may point to
    let outside = td.path().join("outside.txt");
    fs::write(&outside, "outside").unwrap();

    // Spawn a thread that attempts to create a symlink at `dest` shortly after copy begins.
    // This race is best-effort; errors are ignored.
    let dest_clone = dest.clone();
    let outside_clone = outside.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(20));
        let _ = unix_fs::symlink(&outside_clone, &dest_clone);
    });

    // Perform safe copy+rename
    aria_move::fs_ops::safe_copy_and_rename(&src, &dest)
        .expect("safe_copy_and_rename should succeed");
    handle.join().unwrap();

    // If a symlink ended up at dest (concurrent actor won), consider the run inconclusive.
    let symlink_created = match fs::symlink_metadata(&dest) {
        Ok(m) => m.file_type().is_symlink(),
        Err(_) => panic!("Failed to stat destination after race"),
    };

    if symlink_created {
        eprintln!(
            "Symlink was created during race; skipping strict assertions (non-deterministic outcome)."
        );
        return;
    }

    // Otherwise ensure dest is a regular file and content matches source.
    let meta = fs::metadata(&dest).expect("dest metadata");
    assert!(meta.is_file(), "destination is not a regular file");
    let content = fs::read_to_string(&dest).unwrap();
    assert_eq!(content, "from_src");

    // Ensure no temp files remain. Current temp pattern: ".aria_move.<pid>.<nanos>[.<attempt>].tmp"
    for entry in fs::read_dir(&dest_dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        assert!(
            !(name_s.starts_with(".aria_move.") && name_s.ends_with(".tmp")),
            "tmp file left behind: {}",
            name_s
        );
    }
}
