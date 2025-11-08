use assert_cmd::cargo;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

// Helper to write minimal XML config
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
        .arg("TASKID123") // task_id positional
        .arg("1") // num_files positional (legacy form)
        .arg(&missing) // missing source path
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "expected failure status");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Source path not found") || stderr.contains("source_not_found"),
        "unexpected stderr: {stderr}"
    );
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
        .arg("1") // num_files
        .arg(&src) // explicit path
        .output()
        .expect("spawn binary");

    assert!(
        out.status.success(),
        "expected success status; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let dest = completed.join("ok.bin");
    assert!(dest.exists(), "expected file moved to completed");
    assert!(!src.exists(), "source should be gone after move");
}

#[test]
fn three_arg_move_directory_success() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Create directory with nested file
    let dir_src = download.join("dir space");
    fs::create_dir_all(&dir_src).unwrap();
    fs::write(dir_src.join("nested.txt"), b"nested").unwrap();

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID")
        .arg("1")
        .arg(&dir_src) // pass explicit directory path
        .output()
        .expect("spawn binary");

    assert!(
        out.status.success(),
        "expected directory move success; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let dest_dir = completed.join("dir space");
    assert!(dest_dir.exists(), "destination directory should exist");
    assert!(
        dest_dir.join("nested.txt").exists(),
        "nested file should move"
    );
    assert!(!dir_src.exists(), "source directory should be removed");
}

#[cfg(unix)]
#[test]
fn three_arg_move_special_file_rejected() {
    use std::process::Command as Proc;
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    let fifo = download.join("mypipe");
    let status = Proc::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success(), "mkfifo should succeed");

    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASKID")
        .arg("1")
        .arg(&fifo)
        .output()
        .expect("spawn binary");

    assert!(!out.status.success(), "special file should be rejected");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("neither a regular file nor a directory")
            || stderr.contains("Refusing")
            || stderr.contains("Provided path is not a regular file"),
        "unexpected stderr: {stderr}"
    );
}
