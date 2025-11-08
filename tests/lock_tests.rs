use std::fs;

use aria_move::fs_ops::{acquire_dir_lock, acquire_move_lock, try_acquire_dir_lock};

#[test]
fn try_lock_uncontended() {
    let dir = tempfile::tempdir().unwrap();
    let got = try_acquire_dir_lock(dir.path()).unwrap();
    assert!(got.is_some());
}

#[test]
fn try_lock_contended_returns_none() {
    let dir = tempfile::tempdir().unwrap();
    let first = acquire_dir_lock(dir.path()).unwrap();
    let second = try_acquire_dir_lock(dir.path()).unwrap();
    assert!(second.is_none());
    drop(first);
    let third = try_acquire_dir_lock(dir.path()).unwrap();
    assert!(third.is_some());
}

#[test]
fn move_lock_locks_parent_dir() {
    let dir = tempfile::tempdir().unwrap();
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();

    // Acquire move lock on src_dir; this should lock its parent (dir.path())
    let _lock = acquire_move_lock(&src_dir).unwrap();

    // Try to acquire non-blocking lock on parent should fail (None)
    let none = try_acquire_dir_lock(dir.path()).unwrap();
    assert!(none.is_none());
}
