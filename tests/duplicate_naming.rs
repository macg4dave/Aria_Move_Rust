use std::ffi::OsStr;
use std::fs;
use tempfile::tempdir;

use aria_move::fs_ops::{OnDuplicate, resolve_destination};

#[test]
fn no_collision_returns_requested_name() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    let name = OsStr::new("file.txt");
    let dst = resolve_destination(dst_dir, name, OnDuplicate::RenameWithSuffix);
    assert_eq!(dst, dst_dir.join("file.txt"));
}

#[test]
fn single_collision_gets_suffix_two() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    fs::write(dst_dir.join("file.txt"), b"x").unwrap();
    let dst = resolve_destination(
        dst_dir,
        OsStr::new("file.txt"),
        OnDuplicate::RenameWithSuffix,
    );
    assert_eq!(dst, dst_dir.join("file (2).txt"));
}

#[test]
fn multiple_collisions_increment_suffix() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    fs::write(dst_dir.join("file.txt"), b"1").unwrap();
    fs::write(dst_dir.join("file (2).txt"), b"2").unwrap();
    fs::write(dst_dir.join("file (3).txt"), b"3").unwrap();
    let dst = resolve_destination(
        dst_dir,
        OsStr::new("file.txt"),
        OnDuplicate::RenameWithSuffix,
    );
    assert_eq!(dst, dst_dir.join("file (4).txt"));
}

#[test]
fn dotfile_suffixing() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    fs::write(dst_dir.join(".env"), b"a").unwrap();
    let dst = resolve_destination(dst_dir, OsStr::new(".env"), OnDuplicate::RenameWithSuffix);
    assert_eq!(dst, dst_dir.join(".env (2)"));
}

#[test]
fn multi_extension_position() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    fs::write(dst_dir.join("archive.tar.gz"), b"a").unwrap();
    let dst = resolve_destination(
        dst_dir,
        OsStr::new("archive.tar.gz"),
        OnDuplicate::RenameWithSuffix,
    );
    assert_eq!(dst, dst_dir.join("archive.tar (2).gz"));
}

#[test]
fn internal_temp_names_not_suffixed() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    let name = OsStr::new(".aria_move.123.tmp");
    fs::write(dst_dir.join(".aria_move.123.tmp"), b"temp").unwrap();
    let dst = resolve_destination(dst_dir, name, OnDuplicate::RenameWithSuffix);
    assert_eq!(dst, dst_dir.join(".aria_move.123.tmp"));
}

#[cfg(unix)]
#[test]
fn non_utf8_name_suffixing() {
    use std::os::unix::ffi::OsStrExt;
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    // Name with invalid UTF-8 sequence
    let raw = [0xff, 0xfe, b'.', b't', b'x', b't'];
    let name = OsStr::from_bytes(&raw);
    let dst = resolve_destination(dst_dir, name, OnDuplicate::RenameWithSuffix);
    // It should at least return a path inside dst_dir; we can't assert exact string reliably.
    assert!(dst.starts_with(dst_dir));
}

#[test]
fn overwrite_and_skip_return_candidate() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    fs::write(dst_dir.join("thing.bin"), b"x").unwrap();
    let name = OsStr::new("thing.bin");
    assert_eq!(
        resolve_destination(dst_dir, name, OnDuplicate::Overwrite),
        dst_dir.join("thing.bin")
    );
    assert_eq!(
        resolve_destination(dst_dir, name, OnDuplicate::Skip),
        dst_dir.join("thing.bin")
    );
}
