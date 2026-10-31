#[cfg(unix)]
#[path = "common/config_fixture.rs"]
mod fixture;
#[cfg(unix)]
mod unix_quoted {
    use assert_cmd::cargo;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    use super::fixture::write_cfg;

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

        assert!(
            out.status.success(),
            "expected success; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
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

        assert!(
            out.status.success(),
            "expected success; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
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

        assert!(
            out.status.success(),
            "expected quoted single arg directory move success; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let dest = completed.join("Quoted Dir");
        assert!(dest.exists(), "dest directory should exist");
        assert!(dest.join("a.txt").exists(), "inner file should move");
        assert!(!dir.exists(), "source directory should be removed");
    }
    #[test]
    fn literal_quoted_bare_name_wins_over_decoded_name() {
        let td = tempdir().unwrap();
        let base = fs::canonicalize(td.path()).unwrap();
        let download = base.join("incoming");
        let completed = base.join("completed");
        fs::create_dir(&download).unwrap();
        fs::create_dir(&completed).unwrap();
        let config = base.join("config.xml");
        write_cfg(&config, &download, &completed);
        fs::write(download.join("'literal'"), b"quoted").unwrap();
        fs::write(download.join("literal"), b"unquoted").unwrap();
        let output = Command::new(cargo::cargo_bin!("aria_move"))
            .current_dir(&base)
            .env("ARIA_MOVE_CONFIG", &config)
            .arg("'literal'")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(completed.join("'literal'")).unwrap(), b"quoted");
        assert_eq!(fs::read(download.join("literal")).unwrap(), b"unquoted");
    }

    #[test]
    fn explicit_non_unicode_path_is_preserved() {
        use std::os::unix::ffi::OsStringExt;
        let td = tempdir().unwrap();
        let base = fs::canonicalize(td.path()).unwrap();
        let download = base.join("incoming");
        let completed = base.join("completed");
        fs::create_dir(&download).unwrap();
        fs::create_dir(&completed).unwrap();
        let config = base.join("config.xml");
        write_cfg(&config, &download, &completed);
        let name = std::ffi::OsString::from_vec(b"data-\xff".to_vec());
        let source = download.join(&name);
        fs::write(&source, b"bytes").unwrap();
        let output = Command::new(cargo::cargo_bin!("aria_move"))
            .env("ARIA_MOVE_CONFIG", &config)
            .arg("--source-path")
            .arg(&source)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(fs::read(completed.join(name)).unwrap(), b"bytes");
        assert!(!source.exists());
    }
}
