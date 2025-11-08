use serial_test::serial;
use std::fs;
use tempfile::tempdir;

use aria_move::{default_config_path, default_log_path};

#[test]
#[serial]
fn log_colocates_with_env_override_config() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg = base.join("custom_config.xml");
    let download_base = base.join("incoming");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    let xml = format!(
        r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>normal</log_level>
  <preserve_metadata>false</preserve_metadata>
</config>"#,
        download_base.display(),
        completed_base.display()
    );
    fs::write(&cfg, xml).unwrap();

    // Set env for this process; serialize to avoid cross-test interference
    unsafe {
        std::env::set_var("ARIA_MOVE_CONFIG", &cfg);
    }

    // Ensure library respects ARIA_MOVE_CONFIG for config path
    let resolved_cfg = default_config_path().expect("default_config_path");
    assert_eq!(
        resolved_cfg, cfg,
        "config path should equal ARIA_MOVE_CONFIG value"
    );

    // Log path should be colocated with config (same parent dir)
    let resolved_log = default_log_path().expect("default_log_path");
    assert_eq!(
        resolved_log.parent(),
        cfg.parent(),
        "log path parent should match config parent"
    );

    // Cleanup env
    unsafe {
        std::env::remove_var("ARIA_MOVE_CONFIG");
    }
}
