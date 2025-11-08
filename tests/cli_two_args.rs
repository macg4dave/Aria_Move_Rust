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
fn two_args_without_path_errors_and_moves_nothing() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Create a file that should NOT be auto-picked
    let untouched = download.join("should_not_move.bin");
    fs::write(&untouched, b"data").unwrap();

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID_ONLY") // task_id
        .arg("1")           // num_files
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected failure when no explicit path given");
    let stderr = String::from_utf8_lossy(&out.stderr);
    // Program prints simplified Display message; ensure strict no-auto-pick semantics.
    assert!(stderr.contains("No file found under base") || stderr.contains("none_found"), "stderr did not contain expected none_found message: {stderr}");

    // Ensure original file still in download and not moved
    assert!(untouched.exists(), "file should remain in download base");
    assert!(!completed.join("should_not_move.bin").exists(), "file should not have been moved");
}
