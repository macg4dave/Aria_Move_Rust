#[cfg(unix)]
mod unix_quoted {
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
    fn three_arg_single_quoted_directory_with_trailing_slash_moves_ok() {
        let td = tempdir().unwrap();
        let base = fs::canonicalize(td.path()).unwrap();
        let cfg_path = base.join("config.xml");
        let download = base.join("incoming");
        let completed = base.join("completed");
        fs::create_dir_all(&download).unwrap();
        fs::create_dir_all(&completed).unwrap();
        write_cfg(&cfg_path, &download, &completed);

        let dir = download.join("New folder");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("x.txt"), b"hello").unwrap();

        // Simulate a user passing a single-quoted path with trailing slash
        let quoted = format!("'{}'/", dir.display());

        let me = cargo::cargo_bin!("aria_move");
        let out = Command::new(me)
            .env("ARIA_MOVE_CONFIG", &cfg_path)
            .arg("TASK")
            .arg("1")
            .arg(&quoted)
            .output()
            .expect("spawn binary");

        assert!(out.status.success(), "expected success; stderr: {}", String::from_utf8_lossy(&out.stderr));
        let dest_dir = completed.join("New folder");
        assert!(dest_dir.exists());
        assert!(dest_dir.join("x.txt").exists());
        assert!(!dir.exists());
    }

    #[test]
    fn single_arg_double_quoted_file_moves_ok() {
        let td = tempdir().unwrap();
        let base = fs::canonicalize(td.path()).unwrap();
        let cfg_path = base.join("config.xml");
        let download = base.join("incoming");
        let completed = base.join("completed");
        fs::create_dir_all(&download).unwrap();
        fs::create_dir_all(&completed).unwrap();
        write_cfg(&cfg_path, &download, &completed);

        let src = download.join("with space.bin");
        fs::write(&src, b"data").unwrap();
        let quoted = format!("\"{}\"", src.display());

        let me = cargo::cargo_bin!("aria_move");
        let out = Command::new(me)
            .env("ARIA_MOVE_CONFIG", &cfg_path)
            .arg(&quoted)
            .output()
            .expect("spawn binary");

        assert!(out.status.success(), "expected success; stderr: {}", String::from_utf8_lossy(&out.stderr));
        let dest = completed.join("with space.bin");
        assert!(dest.exists());
        assert!(!src.exists());
    }

    #[test]
    fn single_arg_single_quoted_directory_moves_ok() {
        let td = tempdir().unwrap();
        let base = fs::canonicalize(td.path()).unwrap();
        let cfg_path = base.join("config.xml");
        let download = base.join("incoming");
        let completed = base.join("completed");
        fs::create_dir_all(&download).unwrap();
        fs::create_dir_all(&completed).unwrap();
        write_cfg(&cfg_path, &download, &completed);

        let dir = download.join("Quoted Dir");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.txt"), b"A").unwrap();

        // Simulate user entering a quoted directory name (single arg form)
        let quoted = format!("'{}'", dir.display());
        let me = cargo::cargo_bin!("aria_move");
        let out = Command::new(me)
            .env("ARIA_MOVE_CONFIG", &cfg_path)
            .arg(&quoted)
            .output()
            .expect("spawn binary");

        assert!(out.status.success(), "expected quoted single arg directory move success; stderr: {}", String::from_utf8_lossy(&out.stderr));
        let dest = completed.join("Quoted Dir");
        assert!(dest.exists(), "dest directory should exist");
        assert!(dest.join("a.txt").exists(), "inner file should move");
        assert!(!dir.exists(), "source directory should be removed");
    }
}
