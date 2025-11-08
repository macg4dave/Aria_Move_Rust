use aria_move::{Config, load_config_from_xml_path};
use std::fs;
use tempfile::tempdir;

#[test]
fn missing_fields_use_defaults() {
    let td = tempdir().unwrap();
    let cfg_path = td.path().join("config.xml");
    // Only specify one field; others should fall back to defaults
    let xml = r#"<config>
  <download_base>/tmp/incoming</download_base>
</config>"#;
    fs::write(&cfg_path, xml).unwrap();
    let cfg = load_config_from_xml_path(&cfg_path).unwrap();
    assert_eq!(cfg.download_base, std::path::PathBuf::from("/tmp/incoming"));
    // Defaults from Config::default()
    let def = Config::default();
    assert_eq!(cfg.completed_base, def.completed_base);
    assert_eq!(cfg.log_file.is_some(), def.log_file.is_some());
    assert_eq!(cfg.log_level as u8, def.log_level as u8);
}
