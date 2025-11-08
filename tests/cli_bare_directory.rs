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
fn single_arg_bare_directory_moves_from_download_base() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let dname = "New folder"; // includes space
    let src_dir = download.join(dname);
    fs::create_dir_all(&src_dir).unwrap();
    // Add a file inside the directory to verify recursive move
    fs::write(src_dir.join("file.txt"), b"data").unwrap();

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg(dname)
        .output()
        .expect("spawn binary");

    assert!(out.status.success(), "expected success; stderr: {}", String::from_utf8_lossy(&out.stderr));
    let dest_dir = completed.join(dname);
    assert!(dest_dir.exists(), "dest dir should exist");
    assert!(dest_dir.join("file.txt").exists(), "inner file should have moved");
    assert!(!src_dir.exists(), "source dir should be gone");
}

#[test]
fn single_arg_bare_directory_with_trailing_slash_moves_ok() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let dname = "Trailing";
    let src_dir = download.join(dname);
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(src_dir.join("t.txt"), b"T").unwrap();

    // Provide the directory name (bare) with a trailing slash to simulate certain user shells
    let arg = format!("{}/", dname);

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg(&arg)
        .output()
        .expect("spawn binary");

    assert!(out.status.success(), "expected trailing slash directory move success; stderr: {}", String::from_utf8_lossy(&out.stderr));
    let dest_dir = completed.join(dname);
    assert!(dest_dir.exists(), "dest dir should exist");
    assert!(dest_dir.join("t.txt").exists(), "inner file should move");
    assert!(!src_dir.exists(), "source directory should be gone");
}
