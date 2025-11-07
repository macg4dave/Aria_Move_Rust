use std::ffi::OsString;
use std::fs;
use tempfile::tempdir;

use aria_move::fs_ops::{resolve_destination, OnDuplicate};

fn long_name(base: &str, total_len: usize) -> OsString {
    // Build a name "<base_repeated>.txt" with at least total_len chars
    let mut s = String::new();
    while s.len() < total_len.saturating_sub(4) { // leave room for .txt
        s.push_str(base);
    }
    s.truncate(total_len.saturating_sub(4));
    s.push_str(".txt");
    OsString::from(s)
}

#[test]
fn trims_overlong_name_without_collision() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    let name = long_name("a", 400);
    let dst = resolve_destination(dst_dir, &name, OnDuplicate::RenameWithSuffix);
    // Ensure file name is not absurdly long; conservative upper bound 255
    let fname = dst.file_name().unwrap().to_string_lossy();
    assert!(fname.len() <= 255, "filename length should be trimmed to <=255, got {}", fname.len());
    // Since there was no collision, no numeric suffix expected, but trimming may occur.
    assert!(fname.ends_with(".txt"));
}

#[test]
fn trims_and_suffixes_on_collision() {
    let td = tempdir().unwrap();
    let dst_dir = td.path();
    let name = long_name("b", 400);
    // First resolution yields the (possibly trimmed) base candidate
    let first = resolve_destination(dst_dir, &name, OnDuplicate::RenameWithSuffix);
    // Create that file to force a collision on the next resolution
    fs::write(&first, b"x").unwrap();
    let second = resolve_destination(dst_dir, &name, OnDuplicate::RenameWithSuffix);
    let f1 = first.file_name().unwrap().to_string_lossy().into_owned();
    let f2 = second.file_name().unwrap().to_string_lossy().into_owned();
    assert!(f2.ends_with(".txt"));
    assert!(f2.len() <= 255);
    // Expect a numeric suffix like " (2)" before the extension
    assert!(f2.contains(" (2)"), "expected a numeric suffix in '{}', prior was '{}'", f2, f1);
}
