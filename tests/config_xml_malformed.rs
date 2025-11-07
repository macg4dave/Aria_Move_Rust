use std::fs; use tempfile::tempdir; use aria_move::load_config_from_xml_path;

#[test]
fn malformed_xml_errors() {
    let td = tempdir().unwrap();
    let cfg_path = td.path().join("config.xml");
    // Missing closing tag for completed_base
    let xml = r#"<config>
  <download_base>/tmp/incoming</download_base>
  <completed_base>/tmp/completed
</config>"#;
    fs::write(&cfg_path, xml).unwrap();
    let err = load_config_from_xml_path(&cfg_path).unwrap_err();
    assert!(format!("{err}").contains("parse config xml"));
}
