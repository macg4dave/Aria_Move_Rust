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
fn single_arg_bare_filename_moves_from_download_base() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let fname = "bare.txt";
    let src = download.join(fname);
    fs::write(&src, b"x").unwrap();

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg(fname)
        .output()
        .expect("spawn binary");

    assert!(out.status.success(), "expected success; stderr: {}", String::from_utf8_lossy(&out.stderr));
    let dest = completed.join(fname);
    assert!(dest.exists());
    assert!(!src.exists());
}
