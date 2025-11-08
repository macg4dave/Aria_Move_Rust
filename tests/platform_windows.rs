#![cfg(windows)]

use aria_move::platform::open_log_file_secure_append;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn windows_open_log_file_allows_append_and_writes() {
    let td = tempdir().expect("tempdir");
    let log_path = td.path().join("aria_move_windows_test.log");

    // Open and append to the file using the platform helper
    let mut file = open_log_file_secure_append(&log_path).expect("open_log_file_secure_append");
    writeln!(file, "hello windows").expect("write");
    // drop to flush
    drop(file);

    let contents = std::fs::read_to_string(&log_path).expect("read file");
    assert!(contents.contains("hello windows"));
}
