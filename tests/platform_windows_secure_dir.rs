#![cfg(windows)]

use aria_move::platform::ensure_secure_directory;
use std::fs::{self, File};
use tempfile::tempdir;

#[test]
fn windows_ensure_secure_directory_accepts_dir_and_rejects_file_or_readonly() {
    let td = tempdir().expect("tempdir");
    let dir = td.path().join("d");
    fs::create_dir_all(&dir).expect("create dir");

    // Should accept
    ensure_secure_directory(&dir, "test").expect("ensure_secure_directory on dir");

    // Create a file and expect an error
    let file_path = td.path().join("f.txt");
    let mut f = File::create(&file_path).expect("create file");
    use std::io::Write;
    writeln!(f, "x").expect("write");
    drop(f);

    assert!(ensure_secure_directory(&file_path, "test").is_err());

    // Make directory readonly and expect error
    let ro_dir = td.path().join("ro");
    fs::create_dir_all(&ro_dir).expect("create ro dir");
    let mut perms = fs::metadata(&ro_dir).expect("meta").permissions();
    perms.set_readonly(true);
    fs::set_permissions(&ro_dir, perms).expect("set readonly");
    assert!(ensure_secure_directory(&ro_dir, "test").is_err());
}
