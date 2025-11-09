//! Integration test: --config flag overrides ARIA_MOVE_CONFIG env and defaults.
//! Creates two temp config files with different completed_base values and ensures
//! the CLI picks the path passed via --config even when ARIA_MOVE_CONFIG points elsewhere.

use assert_cmd::cargo;
use assert_cmd::assert::OutputAssertExt; // bring .assert() into scope
use std::fs;
use tempfile::tempdir;

// Minimal XML template with differing completed_base so we can detect which was used.
fn write_cfg(path: &std::path::Path, download: &std::path::Path, completed: &std::path::Path) {
    let xml = format!(r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>quiet</log_level>
  <log_file></log_file>
  <preserve_metadata>false</preserve_metadata>
  <preserve_permissions>false</preserve_permissions>
</config>"#, download.display(), completed.display());
    fs::write(path, xml).unwrap();
}

#[test]
fn config_flag_overrides_env() {
    let td = tempdir().unwrap();
    // Canonicalize base to avoid symlink ancestors like /var -> /private/var on macOS.
    let base = fs::canonicalize(td.path()).unwrap();
    let env_cfg = base.join("env_config.xml");
    let flag_cfg = base.join("flag_config.xml");

    let inc_a = base.join("incomingA");
    let com_a = base.join("completedA");
    let inc_b = base.join("incomingB");
    let com_b = base.join("completedB");
    fs::create_dir_all(&inc_a).unwrap();
    fs::create_dir_all(&com_a).unwrap();
    fs::create_dir_all(&inc_b).unwrap();
    fs::create_dir_all(&com_b).unwrap();

    write_cfg(&env_cfg, &inc_a, &com_a);
    write_cfg(&flag_cfg, &inc_b, &com_b);

    // Create a source file under the flag config's download base so resolution succeeds.
    let source = inc_b.join("dummy.bin");
    fs::write(&source, b"data").unwrap();

    let bin = cargo::cargo_bin!("aria_move");
    let mut cmd = std::process::Command::new(&bin);
    // Set ARIA_MOVE_CONFIG to env_cfg, but pass --config flag pointing to flag_cfg.
    cmd.env("ARIA_MOVE_CONFIG", &env_cfg)
        .arg("--config")
        .arg(&flag_cfg)
        .arg("--dry-run")
        .arg(&source);

    // Expect output to mention /tmp/completedB (from flag_cfg) not completedA.
    let output = cmd.assert().success().get_output().stdout.clone();
    let text = String::from_utf8_lossy(&output);
    assert!(text.contains(&com_b.display().to_string()), "stdout should reference flag config completed_base");
    assert!(!text.contains(&com_a.display().to_string()), "stdout should not reference env config completed_base");
}
