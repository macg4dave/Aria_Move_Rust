#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::os::unix::fs as unix_fs;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn safe_copy_and_rename_with_concurrent_symlink_creation() {
    let td = tempdir().unwrap();
    let src = td.path().join("src.txt");
    let mut f = fs::File::create(&src).unwrap();
    write!(f, "from_src").unwrap();
    f.sync_all().unwrap();

    let dest_dir = td.path().join("dest");
    fs::create_dir_all(&dest_dir).unwrap();
    let dest = dest_dir.join("file.txt");

    // create outside file to which a concurrent symlink may point
    let outside = td.path().join("outside.txt");
    fs::write(&outside, "outside").unwrap();

    // spawn a thread that attempts to create a symlink at `dest` shortly after copy begins
    let dest_clone = dest.clone();
    let outside_clone = outside.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        // best-effort race: ignore errors
        let _ = unix_fs::symlink(&outside_clone, &dest_clone);
    });

    // perform safe copy+rename (should produce file with src content)
    aria_move::fs_ops::safe_copy_and_rename(&src, &dest)
        .expect("safe_copy_and_rename should succeed");
    handle.join().unwrap();

    let content = fs::read_to_string(&dest).unwrap();
    assert_eq!(content, "from_src");

    // no tmp files remain
    for entry in fs::read_dir(&dest_dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(
            !name.starts_with(".aria_move.tmp."),
            "tmp file left behind: {}",
            name
        );
    }
}
