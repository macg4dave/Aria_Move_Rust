use std::fs;
use std::process::Command;
use tempfile::tempdir;
// Using macro form to avoid deprecated cargo_bin function

#[test]
fn aria2_positional_input_accepted_and_parsed() {
    let td = tempdir().unwrap();

    // Canonicalize to avoid symlink ancestor problems on macOS
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
</config>"#,
        download_base.display(),
        completed_base.display()
    );
    fs::write(&cfg_path, xml).unwrap();

    // Run binary with aria2-style positional args: TASK_ID NUM_FILES SOURCE_PATH
    let me = assert_cmd::cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("7b3f1234")
        .arg("1")
        .arg(&src)
        .arg("--dry-run")
        .output()
        .expect("spawn binary");

    eprintln!("=== STDOUT ===\n{}", String::from_utf8_lossy(&out.stdout));
    eprintln!("=== STDERR ===\n{}", String::from_utf8_lossy(&out.stderr));

    assert!(out.status.success(), "binary exited with failure");

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("Dry-run: would move") || stdout.contains("Dry-run"),
        "expected dry-run message in stdout; got: {}",
        stdout
    );
}
