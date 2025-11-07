use std::fs;
use std::process::Command;
use tempfile::tempdir;
use assert_cmd::cargo::cargo_bin;

#[test]
fn single_positional_moves_file_to_completed() {
    let td = tempdir().unwrap();

    // Canonicalize to avoid symlink ancestor issues
    let base = fs::canonicalize(td.path()).expect("canonicalize tempdir");

    let cfg_path = base.join("config.xml");
    let download_base = base.join("incoming");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    // Create a source file to "move"
    let src = download_base.join("file.iso");
    fs::write(&src, "fake-iso-content").unwrap();

    // Minimal XML config
    let xml = format!(
        r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>normal</log_level>
  <preserve_metadata>false</preserve_metadata>
  <recent_window_seconds>60</recent_window_seconds>
</config>"#,
        download_base.display(),
        completed_base.display()
    );
    fs::write(&cfg_path, xml).unwrap();

    // Run binary with single positional arg: <SOURCE_PATH>
    let me = cargo_bin("aria_move");
    let out = Command::new(&me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg(&src)
        .output()
        .expect("spawn binary");

    eprintln!("=== STDOUT ===\n{}", String::from_utf8_lossy(&out.stdout));
    eprintln!("=== STDERR ===\n{}", String::from_utf8_lossy(&out.stderr));

    assert!(out.status.success(), "binary exited with failure");

    // File should have been moved into completed_base
    let expected = completed_base.join("file.iso");
    assert!(expected.exists(), "expected file moved to completed: {}", expected.display());
    // Original should not exist
    assert!(!src.exists(), "original source should be moved away");
}
