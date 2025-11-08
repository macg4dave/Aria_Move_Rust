#![cfg(unix)]

use aria_move::platform::{set_dir_mode_0700, set_file_mode_0600};
use std::fs::{self, File};
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn unix_set_dir_and_file_modes() {
    let td = tempdir().expect("tempdir");
    let dir = td.path().join("subdir");
    fs::create_dir_all(&dir).expect("create dir");

    // Make sure dir is writable
    set_dir_mode_0700(&dir).expect("set_dir_mode_0700");
    let dmeta = fs::metadata(&dir).expect("dir meta");
    let dmode = dmeta.permissions().mode() & 0o777;
    assert_eq!(dmode, 0o700, "expected dir mode 0700, got {:o}", dmode);

    // File mode
    let fpath = dir.join("f.txt");
    let mut f = File::create(&fpath).expect("create file");
    use std::io::Write;
    writeln!(f, "hello").expect("write");
    drop(f);

    set_file_mode_0600(&fpath).expect("set_file_mode_0600");
    let fmeta = fs::metadata(&fpath).expect("file meta");
    let fmode = fmeta.permissions().mode() & 0o777;
    assert_eq!(fmode, 0o600, "expected file mode 0600, got {:o}", fmode);
}
