use aria_move::{default_config_path, default_log_path};
#[test]
fn default_log_path_is_in_same_dir_as_config() {
    // default_log_path should be colocated with default_config_path by design.
    let cfg_path = default_config_path().expect("default_config_path");
    let log_path = default_log_path().expect("default_log_path");

    let cfg_parent = cfg_path.parent().expect("cfg parent");
    let log_parent = log_path.parent().expect("log parent");

    assert_eq!(cfg_parent, log_parent, "expected log path to be in same dir as config");
    assert_eq!(log_path.file_name().unwrap(), "aria_move.log");
}
