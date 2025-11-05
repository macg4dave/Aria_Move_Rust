use aria_move::fs_ops::safe_copy_and_rename;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn safe_copy_and_rename_creates_destination_and_cleans_tmp() {
    let td = tempdir().unwrap();
    let src = td.path().join("src.txt");
    let mut f = fs::File::create(&src).unwrap();
    write!(f, "hello world").unwrap();
    f.sync_all().unwrap();

    let dest_dir = td.path().join("destdir");
    fs::create_dir_all(&dest_dir).unwrap();
    let dest = dest_dir.join("dest.txt");

    // perform safe copy
    safe_copy_and_rename(&src, &dest).unwrap();

    // destination exists and content matches
    let content = fs::read_to_string(&dest).unwrap();
    assert_eq!(content, "hello world");

    // tmp files (hidden) should not remain
    let entries = fs::read_dir(&dest_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect::<Vec<_>>();
    assert!(entries.iter().any(|n| n == "dest.txt"));
    // ensure no .aria_move.tmp.* files
    assert!(entries
        .iter()
        .all(|n| !n.to_string_lossy().starts_with(".aria_move.tmp.")));
}

#[test]
fn safe_copy_and_rename_handles_existing_destination_by_replacing() {
    let td = tempdir().unwrap();
    let src = td.path().join("src2.txt");
    let mut f = fs::File::create(&src).unwrap();
    write!(f, "new content").unwrap();
    f.sync_all().unwrap();

    let dest_dir = td.path().join("destdir2");
    fs::create_dir_all(&dest_dir).unwrap();
    let dest = dest_dir.join("dest2.txt");

    // precreate dest with older content
    fs::write(&dest, "old").unwrap();

    safe_copy_and_rename(&src, &dest).unwrap();
    let content = fs::read_to_string(&dest).unwrap();
    assert_eq!(content, "new content");
}
