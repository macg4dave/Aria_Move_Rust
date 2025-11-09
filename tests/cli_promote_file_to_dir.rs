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
fn passing_file_under_base_moves_entire_top_folder() {
    let td = tempdir().unwrap();
    let base = fs::canonicalize(td.path()).unwrap();
    let cfg_path = base.join("config.xml");
    let download = base.join("incoming");
    let completed = base.join("completed");
    fs::create_dir_all(&download).unwrap();
    fs::create_dir_all(&completed).unwrap();
    write_cfg(&cfg_path, &download, &completed);

    // Build a multi-file layout under download_base/rootname
    let root = download.join("rootname");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    let f1 = root.join("a.txt");
    let f2 = sub.join("b.txt");
    fs::write(&f1, b"A").unwrap();
    fs::write(&f2, b"B").unwrap();

    // Pass a path to a file inside the folder; CLI should promote to moving the folder.
    let me = cargo::cargo_bin!("aria_move");
    let out = Command::new(me)
        .env("ARIA_MOVE_CONFIG", &cfg_path)
        .arg("TASK")
        .arg("2")
        .arg(&f2)
        .output()
        .expect("spawn binary");

    assert!(out.status.success(), "expected success; stderr: {}", String::from_utf8_lossy(&out.stderr));

    let dest_dir = completed.join("rootname");
    assert!(dest_dir.exists());
    assert!(dest_dir.join("a.txt").exists());
    assert!(dest_dir.join("sub/b.txt").exists());
    assert!(!root.exists());
}
