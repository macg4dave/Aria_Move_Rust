#![cfg(all(unix, feature = "xattrs"))]

use std::fs;
use std::io::Write;
use std::path::Path;

use aria_move::Config;
use aria_move::fs_ops::{move_dir, safe_copy_and_rename_with_metadata};
use tempfile::tempdir;

fn write_file(path: &Path, contents: &str) {
    let mut f = fs::File::create(path).expect("create source file");
    write!(f, "{}", contents).expect("write source content");
    f.sync_all().expect("sync source file");
}

#[test]
fn xattrs_preserved_on_safe_copy() {
    let td = tempdir().unwrap();
    let src = td.path().join("src.txt");
    write_file(&src, "hello");

    // set an extended attribute on source
    xattr::set(&src, "user.test", b"world").expect("set xattr on src");

    let dest_dir = td.path().join("dest");
    fs::create_dir_all(&dest_dir).unwrap();
    let dest = dest_dir.join("dst.txt");

    // Global preserve_metadata enables both regular metadata and xattrs
    safe_copy_and_rename_with_metadata(&src, &dest, true).expect("copy with xattrs");

    let val = xattr::get(&dest, "user.test").expect("get xattr from dest");
    assert_eq!(val.as_deref(), Some(b"world".as_slice()));
}

#[test]
fn xattrs_preserved_on_dir_move_copy_fallback() {
    // Force copy fallback via test-only env var (unsafe on Rust 2024 due to global process env)
    unsafe { std::env::set_var("ARIA_MOVE_FORCE_DIR_COPY", "1") };

    let download = tempdir().unwrap();
    let completed = tempdir().unwrap();

    let cfg = Config {
        download_base: download.path().into(),
        completed_base: completed.path().into(),
        preserve_metadata: true, // preserve everything
        ..Config::default()
    };

    let src_dir = download.path().join("tree");
    fs::create_dir_all(&src_dir).unwrap();
    let f = src_dir.join("file.bin");
    write_file(&f, "data");
    xattr::set(&f, "user.copy", b"me").expect("set xattr on file");

    let dest_dir = move_dir(&cfg, &src_dir).expect("move_dir copy fallback");
    let moved = dest_dir.join("file.bin");
    assert!(moved.exists(), "moved file missing");

    let got = xattr::get(&moved, "user.copy").expect("get xattr dest");
    assert_eq!(got.as_deref(), Some(b"me".as_slice()));
}
