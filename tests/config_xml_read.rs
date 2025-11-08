//! Verify XML config is parsed and used without touching user state.

use std::fs;
use tempfile::tempdir;

use aria_move::{LogLevel, load_config_from_xml_path};

#[test]
fn reads_config_xml_and_applies_values() {
    let td = tempdir().expect("create tempdir");

    let cfg_path = td.path().join("config.xml");
    let download_base = td.path().join("downloads");
    let completed_base = td.path().join("completed");
    let log_file = td.path().join("aria_move.log");

    let xml = format!(
        r#"
<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>normal</log_level>
  <log_file>{}</log_file>
  <preserve_metadata>true</preserve_metadata>
</config>
"#,
        download_base.display(),
        completed_base.display(),
        log_file.display()
    );
    fs::write(&cfg_path, xml).expect("write config.xml");

    // Load config directly from the XML path and assert fields.
    let cfg = load_config_from_xml_path(&cfg_path).expect("load_config_from_xml_path");

    assert_eq!(cfg.download_base, download_base, "download_base mismatch");
    assert_eq!(
        cfg.completed_base, completed_base,
        "completed_base mismatch"
    );
    assert_eq!(
        cfg.log_file.as_deref(),
        Some(log_file.as_path()),
        "log_file mismatch"
    );
    assert_eq!(cfg.log_level, LogLevel::Normal, "log_level mismatch");
    assert!(cfg.preserve_metadata, "preserve_metadata should be true");
    // auto-pick window removed; no assertion for recency.
}
