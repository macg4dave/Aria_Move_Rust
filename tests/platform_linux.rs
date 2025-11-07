#![cfg(target_os = "linux")]

use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;
use aria_move::platform::open_log_file_secure_append;

#[test]
fn linux_open_log_file_sets_0600_mode() {
    let td = tempdir().expect("tempdir");
    let log_path = td.path().join("aria_move_linux_test.log");

    // Ensure the helper can open/create the file
    let file = open_log_file_secure_append(&log_path).expect("open_log_file_secure_append");
    drop(file);

    let meta = std::fs::metadata(&log_path).expect("metadata");
    let mode = meta.permissions().mode() & 0o777;

    assert_eq!(mode, 0o600, "expected file mode 0600 on Linux, got {:o}", mode);
}
