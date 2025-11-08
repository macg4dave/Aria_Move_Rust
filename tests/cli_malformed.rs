use std::fs;use std::process::Command;use tempfile::tempdir;use assert_cmd::cargo;

fn write_cfg(path: &std::path::Path, download: &std::path::Path, completed: &std::path::Path) {
    let xml = format!(r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>quiet</log_level>
  <preserve_metadata>false</preserve_metadata>
</config>"#, download.display(), completed.display());
    fs::write(path, xml).unwrap();
}

#[test]
fn non_numeric_num_files_is_rejected_by_cli() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID")
        .arg("not-a-number") // invalid usize for num_files
        .arg(download.join("file.bin"))
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected parse failure");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // clap typically reports invalid value errors
    assert!(stderr.contains("invalid value") || stderr.contains("error:"), "stderr did not report invalid value: {stderr}");
}

#[test]
fn too_many_args_are_rejected_by_cli() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("A")
        .arg("1")
        .arg(download.join("f.bin"))
        .arg("EXTRA") // unexpected
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected clap to reject extra args");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("unexpected") || stderr.contains("Usage") || stderr.contains("error:"), "stderr did not indicate too many args: {stderr}");
}
