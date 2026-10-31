mod common;
use aria_move::load_config_from_xml_path;
use std::fs;
use tempfile::tempdir;

#[test]
fn empty_log_file_leaves_default_intact() {
    let td = tempdir().unwrap();
    let cfg_path = td.path().join("config.xml");
    let xml = r#"<config>
  <download_base>/tmp/incoming</download_base>
  <completed_base>/tmp/completed</completed_base>
  <log_file></log_file>
</config>"#;
    fs::write(&cfg_path, xml).unwrap();
    let cfg = load_config_from_xml_path(&cfg_path).unwrap();
    // With empty <log_file></log_file> we fall back to global default (default_log_path())
    let expected = aria_move::default_log_path().unwrap();
    assert_eq!(
        cfg.log_file.as_ref().map(|p| p.display().to_string()),
        Some(expected.display().to_string())
    );
}

#[test]
fn empty_log_file_leaves_default_intact_in_global_loader() {
    common::config_environment(
        "empty_log_file_leaves_default_intact_in_global_loader",
        false,
        |root| {
            let cfg_path = root.join("config.xml");
            std::fs::write(&cfg_path, "<config><log_file></log_file></config>").unwrap();
            let cfg = aria_move::load_config_from_xml_env().unwrap().unwrap();
            assert_eq!(cfg.log_file, Some(root.join("aria_move.log")));
        },
    );
}
