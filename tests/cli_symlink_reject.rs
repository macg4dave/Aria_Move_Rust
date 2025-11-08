#![cfg(unix)]

use assert_cmd::cargo;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::process::Command;
use tempfile::tempdir;

fn write_cfg(path: &std::path::Path, download: &std::path::Path, completed: &std::path::Path) {
    let xml = format!(
        r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>quiet</log_level>
  <preserve_metadata>false</preserve_metadata>
</config>"#,
        download.display(),
        completed.display()
    );
    fs::write(path, xml).unwrap();
}

#[test]
fn single_arg_bare_symlink_rejected() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap(); // avoid ambient symlinks in ancestors
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Real target dir outside the download dir
    let real_dir = base.join("real_dir");
    fs::create_dir_all(&real_dir).unwrap();

    // Create a symlink under download that points to real_dir
    let link_name = "dir_link";
    let link_path = download.join(link_name);
    unix_fs::symlink(&real_dir, &link_path).unwrap();

    // Invoke with bare name; resolver will look under download_base and find the symlink
    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg(link_name)
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected symlink to be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Refusing to move symlink")
            || stderr.contains("refusing")
            || stderr.contains("symlink"),
        "unexpected stderr: {stderr}"
    );
}

#[test]
fn explicit_path_symlink_rejected() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Real file target; link points to it
    let real_file = base.join("real.bin");
    fs::write(&real_file, b"x").unwrap();
    let link_path = download.join("file_link");
    unix_fs::symlink(&real_file, &link_path).unwrap();

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg(&link_path)
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected symlink to be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Refusing to move symlink")
            || stderr.contains("refusing")
            || stderr.contains("symlink"),
        "unexpected stderr: {stderr}"
    );
}
