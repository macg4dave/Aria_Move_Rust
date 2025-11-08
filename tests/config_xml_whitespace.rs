use std::fs; use tempfile::tempdir; use aria_move::{load_config_from_xml_path, LogLevel};

#[test]
fn trims_whitespace_in_xml_values() {
    let td = tempdir().unwrap();
    let cfg_path = td.path().join("config.xml");
    let download = td.path().join("downloads");
    let completed = td.path().join("completed");
  let log_file = td.path().join("aria_move.log");

    let xml = format!(r#"<config>
  <download_base>  {download}  </download_base>
  <completed_base>
   {completed}
  </completed_base>
  <log_level>  info  </log_level>
  <log_file>  {lf}  </log_file>
</config>"#, download=download.display(), completed=completed.display(), lf=log_file.display());

    fs::write(&cfg_path, xml).unwrap();
    let cfg = load_config_from_xml_path(&cfg_path).unwrap();
    assert_eq!(cfg.download_base, download);
    assert_eq!(cfg.completed_base, completed);
    assert_eq!(cfg.log_level, LogLevel::Info);
  // recent_window is not configurable via XML; it should remain default (300s)
  assert_eq!(cfg.recent_window.as_secs(), 300);
  assert_eq!(cfg.log_file.as_ref().unwrap().display().to_string(), log_file.display().to_string());
}
