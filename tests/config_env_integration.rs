use std::fs;
use std::process::Command;
use tempfile::tempdir;
use assert_cmd::cargo::cargo_bin; // keep import for macro re-export

#[test]
fn binary_uses_config_pointed_by_env() {
    let td = tempdir().unwrap();

    // Canonicalize to resolve /var -> /private/var on macOS and avoid symlink ancestors
    let base = fs::canonicalize(td.path()).expect("canonicalize tempdir");

    let cfg_path = base.join("config.xml");
    let download_base = base.join("downloads");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    // Create a source file to "move"
    let src = download_base.join("test.txt");
    fs::write(&src, "hello").unwrap();

    // Write minimal XML config with canonicalized paths
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

    eprintln!("Test config path: {}", cfg_path.display());
    eprintln!("Download base: {}", download_base.display());
    eprintln!("Completed base: {}", completed_base.display());
    eprintln!("Source file: {}", src.display());

    // Run with ARIA_MOVE_CONFIG and --dry-run
    let me = cargo_bin!("aria_move");
    let out = Command::new(&me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("--dry-run")
        .arg("--source-path")
        .arg(&src)
        .output()
        .expect("spawn binary");

    eprintln!("Binary: {}", me.display());
    eprintln!("Exit status: {:?}", out.status);
    eprintln!("=== STDOUT ===\n{}", String::from_utf8_lossy(&out.stdout));
    eprintln!("=== STDERR ===\n{}", String::from_utf8_lossy(&out.stderr));

    if !out.status.success() {
        panic!("binary exited with failure");
    }

    // Verify dry-run behavior: source still exists, destination does not
    assert!(src.exists(), "source should still exist with dry-run");
    let expected_dest = completed_base.join("test.txt");
    assert!(
        !expected_dest.exists(),
        "destination should not exist with dry-run"
    );
}