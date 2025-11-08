use std::fs;use std::process::Command;use tempfile::tempdir;use assert_cmd::cargo;

// Helper to write minimal XML config
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
fn three_arg_missing_path_errors() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Intentionally do NOT create the source file
    let missing = download.join("nope.bin");

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID123")   // task_id positional
        .arg("1")           // num_files positional (legacy form)
        .arg(&missing)       // missing source path
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected failure status");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("Source path not found") || stderr.contains("source_not_found"), "unexpected stderr: {stderr}");
}

#[test]
fn three_arg_move_success() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Create the source file
    let src = download.join("ok.bin");
    fs::write(&src, b"content").unwrap();

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("ABCDEF") // task_id
        .arg("1")      // num_files
        .arg(&src)      // explicit path
        .output()
        .expect("spawn binary");

    assert!(out.status.success(), "expected success status; stderr: {}", String::from_utf8_lossy(&out.stderr));

    let dest = completed.join("ok.bin");
    assert!(dest.exists(), "expected file moved to completed");
    assert!(!src.exists(), "source should be gone after move");
}
