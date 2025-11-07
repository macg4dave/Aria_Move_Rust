use std::fs;
use std::process::Command;
use tempfile::tempdir;
// Use macro to avoid deprecated cargo_bin function
use serial_test::serial;

// This test uses a relative ARIA_MOVE_CONFIG path; it MUST run in isolation because it changes CWD.
#[test]
#[serial]
fn relative_env_config_path_resolved() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();

    // Change current directory to temp base for relative path resolution
    std::env::set_current_dir(&base).expect("chdir to temp base");

    let rel_cfg = "rel_config.xml"; // relative path
    let download_base = base.join("incoming");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base).unwrap();
    fs::create_dir_all(&completed_base).unwrap();

    let cfg_full = base.join(rel_cfg);
    let xml = format!(r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>normal</log_level>
  <preserve_metadata>false</preserve_metadata>
  <recent_window_seconds>60</recent_window_seconds>
</config>"#, download_base.display(), completed_base.display());
    fs::write(&cfg_full, xml).unwrap();

    let src = download_base.join("file.bin");
    fs::write(&src, "bin data").unwrap();

    let me = assert_cmd::cargo::cargo_bin!("aria_move");
    let out = Command::new(&me)
        .env("ARIA_MOVE_CONFIG", rel_cfg)
        .arg(&src)
        .output()
        .expect("run binary");

    eprintln!("STDOUT:\n{}", String::from_utf8_lossy(&out.stdout));
    eprintln!("STDERR:\n{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "binary failed");

    // After resolution the config should be treated as absolute (since we joined CWD)
    assert!(cfg_full.exists(), "expected resolved config file to exist at {}", cfg_full.display());

    // File moved
    let moved = completed_base.join("file.bin");
    assert!(moved.exists(), "expected file moved");
}
