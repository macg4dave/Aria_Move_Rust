use aria_move::{default_config_path, default_log_path};
use serial_test::serial;
use std::fs;
use tempfile::tempdir;

#[test]
#[serial]
fn env_override_directory_appends_config_xml_and_colocates_log() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();

    // Set ARIA_MOVE_CONFIG to the directory path (not a file)
    unsafe {
        std::env::set_var("ARIA_MOVE_CONFIG", &base);
    }

    let cfg_path = default_config_path().expect("default_config_path");
    assert!(
        cfg_path.ends_with("config.xml"),
        "expected config.xml appended; got {}",
        cfg_path.display()
    );
    assert_eq!(
        cfg_path.parent().unwrap(),
        base.as_path(),
        "config.xml should be inside the provided directory"
    );

    let log_path = default_log_path().expect("default_log_path");
    assert_eq!(
        log_path.parent().unwrap(),
        base.as_path(),
        "log file should colocate inside the provided directory"
    );

    // Cleanup
    unsafe {
        std::env::remove_var("ARIA_MOVE_CONFIG");
    }
}
