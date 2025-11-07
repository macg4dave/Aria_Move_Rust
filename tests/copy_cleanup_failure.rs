#![cfg(unix)]
use std::fs; use std::os::unix::fs::PermissionsExt; use tempfile::tempdir; use aria_move::fs_ops::safe_copy_and_rename;

#[test]
fn tmp_is_cleaned_on_rename_failure() {
    let td = tempdir().unwrap();
    let src = td.path().join("src.txt");
    fs::write(&src, "hello").unwrap();

    let dest_dir = td.path().join("readonly");
    fs::create_dir_all(&dest_dir).unwrap();
    // Make directory read-only (remove write bit) to force rename failure
    let mut perms = fs::metadata(&dest_dir).unwrap().permissions();
    perms.set_mode(0o555);
    fs::set_permissions(&dest_dir, perms).unwrap();

    let dest = dest_dir.join("file.txt");
    let res = safe_copy_and_rename(&src, &dest);
    assert!(res.is_err(), "expected error due to readonly directory");

    // Ensure no temp files remain with pattern .aria_move.*.tmp
    let entries = fs::read_dir(&dest_dir).unwrap();
    for e in entries {
        let name = e.unwrap().file_name();
        let s = name.to_string_lossy();
        assert!(
            !(s.starts_with(".aria_move.") && s.ends_with(".tmp")),
            "tmp file left behind: {}",
            s
        );
    }
}
