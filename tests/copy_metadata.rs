use std::fs; use tempfile::tempdir; use aria_move::fs_ops::safe_copy_and_rename_with_metadata; use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
#[test]
fn copy_preserves_metadata_when_requested() {
    let td = tempdir().unwrap();
    let src = td.path().join("src_meta.txt");
    fs::write(&src, "contents").unwrap();
    // Change mode
    let mut perms = fs::metadata(&src).unwrap().permissions();
    perms.set_mode(0o640);
    fs::set_permissions(&src, perms).unwrap();

    let dest_dir = td.path().join("destm");
    fs::create_dir_all(&dest_dir).unwrap();
    let dest = dest_dir.join("dest_meta.txt");

    safe_copy_and_rename_with_metadata(&src, &dest, true).unwrap();
    let meta = fs::metadata(&dest).unwrap();
    assert_eq!(meta.permissions().mode() & 0o777, 0o640);
}
