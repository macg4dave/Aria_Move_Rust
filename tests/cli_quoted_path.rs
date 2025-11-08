use assert_cmd::cargo;
use std::fs;
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
fn three_arg_single_quoted_dir_with_trailing_backslash_moves_ok() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Create a directory with a space in its name and a file inside it
    let dir = download.join("New folder");
    fs::create_dir_all(&dir).unwrap();
    let inner = dir.join("file.txt");
    fs::write(&inner, b"data").unwrap();

    // Simulate PowerShell single-quoted argument with trailing backslash
    // The quotes become part of argv when using Command::arg, which simulates user quoting.
    let quoted = format!("'{}\\'", dir.display());

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID")
        .arg("1")
        .arg(&quoted)
        .output()
        .expect("spawn binary");

    assert!(
        out.status.success(),
        "expected success; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    // Directory should be moved under completed with same name
    let dest_dir = completed.join("New folder");
    assert!(
        dest_dir.exists(),
        "dest dir should exist: {}",
        dest_dir.display()
    );
    assert!(
        dest_dir.join("file.txt").exists(),
        "inner file should have moved"
    );
    assert!(!dir.exists(), "source dir should be gone");
}

#[test]
fn three_arg_double_quoted_file_moves_ok() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let src = download.join("with space.bin");
    fs::write(&src, b"content").unwrap();

    // Simulate double-quoted argv
    let quoted = format!("\"{}\"", src.display());

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID")
        .arg("1")
        .arg(&quoted)
        .output()
        .expect("spawn binary");

    assert!(
        out.status.success(),
        "expected success; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dest = completed.join("with space.bin");
    assert!(dest.exists(), "dest should exist");
    assert!(!src.exists(), "src should be moved");
}
