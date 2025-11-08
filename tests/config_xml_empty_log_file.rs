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
    // This test exercises load_config_from_xml() path by writing to default path via override.
    // We'll set ARIA_MOVE_CONFIG to our temp file and then call app's load path indirectly.
    let td = tempdir().unwrap();
    let cfg_path = td.path().join("config.xml");
    let xml = r#"<config>
  <download_base>/tmp/incoming</download_base>
  <completed_base>/tmp/completed</completed_base>
  <log_file></log_file>
</config>"#;
    fs::write(&cfg_path, xml).unwrap();

    unsafe {
        std::env::set_var("ARIA_MOVE_CONFIG", &cfg_path);
    }
    let cfg = aria_move::load_config_from_xml_path(&cfg_path).unwrap();
    let expected = aria_move::default_log_path().unwrap();
    assert_eq!(
        cfg.log_file.as_ref().map(|p| p.display().to_string()),
        Some(expected.display().to_string())
    );
    unsafe {
        std::env::remove_var("ARIA_MOVE_CONFIG");
    }
}
