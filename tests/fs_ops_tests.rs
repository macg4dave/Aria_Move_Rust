use aria_move::fs_ops::safe_copy_and_rename;
use std::fs;
use std::io::Write;
use tempfile::tempdir;

/// Create a file with the given content and fsync it (to avoid flakiness in tests).
fn create_file_with_content(path: &std::path::Path, content: &str) {
    let mut f = fs::File::create(path).expect("create source file");
    f.write_all(content.as_bytes()).expect("write source content");
    f.sync_all().expect("sync source file");
}

#[test]
fn safe_copy_and_rename_creates_destination_and_cleans_tmp() {
    let td = tempdir().unwrap();

    // Source file
    let src = td.path().join("src.txt");
    create_file_with_content(&src, "hello world");

    // Destination dir and path
    let dest_dir = td.path().join("destdir");
    fs::create_dir_all(&dest_dir).expect("create dest dir");
    let dest = dest_dir.join("dest.txt");

    // Perform safe copy
    safe_copy_and_rename(&src, &dest).expect("safe_copy_and_rename");

    // Destination exists and content matches
    assert!(dest.is_file(), "destination file not created");
    let content = fs::read_to_string(&dest).expect("read destination");
    assert_eq!(content, "hello world");

    // Ensure no temp files remain.
    // Current temp pattern: ".aria_move.<pid>.<nanos>[.<attempt>].tmp"
    for entry in fs::read_dir(&dest_dir).expect("list dest dir") {
        let entry = entry.expect("dir entry");
        let name = entry.file_name();
        let name_s = name.to_string_lossy();
        assert!(
            !(name_s.starts_with(".aria_move.") && name_s.ends_with(".tmp")),
            "tmp file left behind: {}",
            name_s
        );
    }
}

#[test]
fn safe_copy_and_rename_handles_existing_destination_by_replacing() {
    let td = tempdir().unwrap();

    let src = td.path().join("src2.txt");
    create_file_with_content(&src, "new content");

    let dest_dir = td.path().join("destdir2");
    fs::create_dir_all(&dest_dir).expect("create dest dir");
    let dest = dest_dir.join("dest2.txt");

    // Precreate destination with older content
    fs::write(&dest, "old").expect("precreate destination");

    // Should overwrite existing file
    safe_copy_and_rename(&src, &dest).expect("safe_copy_and_rename overwrite");
    let content = fs::read_to_string(&dest).expect("read destination");
    assert_eq!(content, "new content");
}
