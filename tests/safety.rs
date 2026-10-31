use aria_move::{Config, config, fs_ops};
use std::fs;
use std::path::Path;
use std::process::Command;

fn snapshot(root: &Path) -> Vec<(std::path::PathBuf, Option<Vec<u8>>)> {
    let mut entries: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| {
            let entry = entry.unwrap();
            let contents = entry
                .file_type()
                .is_file()
                .then(|| fs::read(entry.path()).unwrap());
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                contents,
            )
        })
        .collect();
    entries.sort();
    entries
}

#[test]
fn direct_dry_runs_create_nothing_even_with_missing_destination() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let incoming = root.join("incoming");
    let completed = root.join("missing/nested/completed");
    let directory = incoming.join("folder/empty");
    fs::create_dir_all(&directory)?;
    let file = incoming.join("file");
    fs::write(&file, b"bytes")?;
    let mut cfg = Config::new(&incoming, &completed);
    cfg.dry_run = true;
    let before = snapshot(&root);
    config::validate_and_normalize(&mut cfg)?;
    fs_ops::move_file(&cfg, &file)?;
    fs_ops::move_dir(&cfg, directory.parent().unwrap())?;
    assert_eq!(snapshot(&root), before);
    Ok(())
}

#[test]
fn cli_dry_run_keeps_stale_temps_and_unrelated_partial_looking_trees() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let incoming = root.join("incoming");
    let completed = root.join("completed");
    fs::create_dir_all(incoming.join("movie"))?;
    fs::create_dir_all(completed.join("movie"))?;
    fs::write(incoming.join("movie/a"), b"source a")?;
    fs::write(incoming.join("movie/b"), b"source b")?;
    fs::write(completed.join("movie/unrelated"), b"do not delete")?;
    fs::write(completed.join(".aria_move.resume.deadbeef.tmp"), b"active")?;
    let config = root.join("config.xml");
    let xml = format!(
        "<config><download_base>{}</download_base><completed_base>{}</completed_base></config>",
        incoming.display(),
        completed.display()
    );
    fs::write(&config, xml)?;
    let before = snapshot(&root);
    let result = Command::new(assert_cmd::cargo::cargo_bin!("aria_move"))
        .args(["--dry-run", "--config"])
        .arg(config)
        .arg(incoming.join("movie"))
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(snapshot(&root), before);
    Ok(())
}

#[test]
fn missing_config_dry_run_and_invalid_nested_config_write_nothing() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let before = snapshot(&root);
    let result = Command::new(assert_cmd::cargo::cargo_bin!("aria_move"))
        .args(["--dry-run", "--config"])
        .arg(root.join("missing/config.xml"))
        .output()?;
    assert!(!result.status.success());
    assert_eq!(snapshot(&root), before);
    let mut cfg = Config::new(root.join("missing"), root.join("missing/child"));
    assert!(config::validate_and_normalize(&mut cfg).is_err());
    assert_eq!(snapshot(&root), before);
    Ok(())
}

#[test]
fn concurrent_publication_has_exactly_one_winner() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let a = td.path().join("a");
    let b = td.path().join("b");
    let dest = td.path().join("destination");
    fs::write(&a, vec![1; 1024 * 1024])?;
    fs::write(&b, vec![2; 1024 * 1024])?;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles: Vec<_> = [a.clone(), b.clone()]
        .into_iter()
        .map(|src| {
            let barrier = barrier.clone();
            let dest = dest.clone();
            std::thread::spawn(move || {
                barrier.wait();
                fs_ops::safe_copy_and_rename(&src, &dest).is_ok()
            })
        })
        .collect();
    let outcomes: Vec<_> = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect();
    assert_eq!(outcomes.iter().filter(|&&success| success).count(), 1);
    let expected = if outcomes[0] {
        fs::read(&a)?
    } else {
        fs::read(&b)?
    };
    assert_eq!(fs::read(&dest)?, expected);
    assert!(a.exists() && b.exists());
    assert_eq!(fs::read_dir(td.path())?.count(), 3);
    Ok(())
}

#[test]
fn config_writer_never_replaces_and_preserves_existing_mode() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let path = td.path().join("config.xml");
    aria_move::platform::write_config_secure_new_0600(&path, b"original")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))?;
        fs::set_permissions(td.path(), fs::Permissions::from_mode(0o750))?;
    }
    assert!(aria_move::platform::write_config_secure_new_0600(&path, b"replace").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(fs::metadata(&path)?.permissions().mode() & 0o777, 0o640);
        assert_eq!(fs::metadata(td.path())?.permissions().mode() & 0o777, 0o750);
    }
    assert_eq!(fs::read(path)?, b"original");
    assert_eq!(fs::read_dir(td.path())?.count(), 1);
    Ok(())
}

#[test]
fn malformed_unknown_and_boolean_only_configuration_is_consistent() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let path = td.path().join("config.xml");
    for xml in [
        "<config><unknown>true</unknown></config>",
        "<config><disable_locks>broken</disable_locks></config>",
    ] {
        fs::write(&path, xml)?;
        assert!(config::xml::load_config_from_xml_path(&path).is_err());
    }
    fs::write(
        &path,
        "<config><disable_locks>true</disable_locks><preserve_metadata>true</preserve_metadata><preserve_permissions>true</preserve_permissions></config>",
    )?;
    let cfg = config::xml::load_config_from_xml_path(&path)?;
    assert!(cfg.disable_locks && cfg.preserve_metadata);
    assert!(!cfg.preserve_permissions);
    Ok(())
}

#[cfg(unix)]
#[test]
fn tree_links_and_special_entries_are_rejected_without_deletion() -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let incoming = root.join("incoming");
    let source = incoming.join("tree");
    let completed = root.join("completed");
    fs::create_dir_all(&source)?;
    fs::create_dir(&completed)?;
    fs::write(source.join("ordinary"), b"keep")?;
    let link = source.join("link");
    symlink(root.join("missing"), &link)?;
    let cfg = Config::new(&incoming, &completed);
    assert!(fs_ops::move_dir(&cfg, &source).is_err());
    fs::remove_file(&link)?;
    assert!(
        Command::new("mkfifo")
            .arg(source.join("fifo"))
            .status()?
            .success()
    );
    assert!(fs_ops::move_dir(&cfg, &source).is_err());
    assert_eq!(fs::read(source.join("ordinary"))?, b"keep");
    assert!(fs::read_dir(&completed)?.next().is_none());
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn native_cross_device_file_and_directory_moves_are_complete() -> anyhow::Result<()> {
    use std::os::unix::fs::MetadataExt;
    let source_root = tempfile::tempdir()?;
    // /dev/shm is a separate temporary filesystem on Linux. This test has no
    // real-storage configuration or external network dependency.
    let Ok(destination_root) = tempfile::tempdir_in("/dev/shm") else {
        return Ok(());
    };
    if fs::metadata(source_root.path())?.dev() == fs::metadata(destination_root.path())?.dev() {
        return Ok(());
    }
    let file = source_root.path().join("file");
    let tree = source_root.path().join("tree");
    fs::write(&file, b"file bytes")?;
    fs::create_dir_all(tree.join("nested/empty"))?;
    fs::write(tree.join("nested/child"), b"child bytes")?;
    let mut cfg = Config::new(source_root.path(), destination_root.path());
    cfg.preserve_metadata = true;
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&file, fs::Permissions::from_mode(0o640))?;
    fs::set_permissions(&tree, fs::Permissions::from_mode(0o750))?;
    let modified = filetime::FileTime::from_unix_time(1_700_000_000, 0);
    filetime::set_file_mtime(&file, modified)?;
    #[cfg(feature = "xattrs")]
    xattr::set(tree.join("nested/child"), "user.aria_move", b"attribute")?;
    let dest_file = fs_ops::move_file(&cfg, &file)?;
    let dest_tree = fs_ops::move_dir(&cfg, &tree)?;
    assert_eq!(
        fs::metadata(&dest_file)?.permissions().mode() & 0o777,
        0o640
    );
    assert_eq!(
        filetime::FileTime::from_last_modification_time(&fs::metadata(&dest_file)?),
        modified
    );
    assert_eq!(
        fs::metadata(&dest_tree)?.permissions().mode() & 0o777,
        0o750
    );
    #[cfg(feature = "xattrs")]
    assert_eq!(
        xattr::get(dest_tree.join("nested/child"), "user.aria_move")?,
        Some(b"attribute".to_vec())
    );
    assert_eq!(fs::read(dest_file)?, b"file bytes");
    assert_eq!(fs::read(dest_tree.join("nested/child"))?, b"child bytes");
    assert!(dest_tree.join("nested/empty").is_dir());
    assert!(!file.exists() && !tree.exists());
    Ok(())
}

#[test]
fn json_dry_run_has_structured_stdout_and_no_default_log() -> anyhow::Result<()> {
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let incoming = root.join("incoming");
    let completed = root.join("completed");
    fs::create_dir(&incoming)?;
    fs::write(incoming.join("file"), b"keep")?;
    let path = root.join("config.xml");
    fs::write(
        &path,
        format!(
            "<config><download_base>{}</download_base><completed_base>{}</completed_base></config>",
            incoming.display(),
            completed.display()
        ),
    )?;
    let before = snapshot(&root);
    let output = Command::new(assert_cmd::cargo::cargo_bin!("aria_move"))
        .args(["--json", "--dry-run", "--config"])
        .arg(path)
        .arg(incoming.join("file"))
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout)?;
    assert!(!stdout.is_empty());
    for line in stdout.lines() {
        let _: serde_json::Value = serde_json::from_str(line)?;
    }
    assert_eq!(snapshot(&root), before);
    Ok(())
}

#[cfg(unix)]
#[test]
fn sigterm_interrupts_a_cli_waiting_for_a_lock() -> anyhow::Result<()> {
    use std::io::{BufRead, BufReader};
    use std::process::Stdio;
    use std::time::{Duration, Instant};
    let td = tempfile::tempdir()?;
    let root = dunce::canonicalize(td.path())?;
    let incoming = root.join("incoming");
    let completed = root.join("completed");
    fs::create_dir(&incoming)?;
    fs::create_dir(&completed)?;
    let source = incoming.join("file");
    fs::write(&source, b"keep")?;
    let path = root.join("config.xml");
    fs::write(
        &path,
        format!(
            "<config><download_base>{}</download_base><completed_base>{}</completed_base></config>",
            incoming.display(),
            completed.display()
        ),
    )?;
    let held = fs_ops::acquire_dir_lock(&incoming)?;
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("aria_move"))
        .args(["--debug", "--json", "--config"])
        .arg(path)
        .arg(&source)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let reader = std::thread::spawn(move || {
        let mut sender = Some(sender);
        for line in BufReader::new(stdout).lines() {
            let line = line.unwrap();
            if line.contains("Waiting for directory lock")
                && line.contains("incoming")
                && let Some(sender) = sender.take()
            {
                let _ = sender.send(());
            }
        }
    });
    let ready = receiver.recv_timeout(Duration::from_secs(5)).is_ok();
    if ready {
        // SAFETY: this is our live child process ID. SIGTERM is deliberately caught
        // by its configured ctrlc handler; no unrelated process is signalled.
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    } else {
        child.kill()?;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill()?;
            break child.wait()?;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    reader.join().unwrap();
    drop(held);
    assert!(ready, "child never reached cancellable lock acquisition");
    assert_eq!(
        status.code(),
        Some(1),
        "SIGTERM must be handled, not terminate the process directly"
    );
    assert_eq!(fs::read(source)?, b"keep");
    assert!(fs::read_dir(completed)?.next().is_none());
    Ok(())
}

#[test]
fn disk_query_rejects_embedded_nul_instead_of_querying_a_truncated_path() {
    let path = std::path::PathBuf::from(".\0unexpected");
    let error = aria_move::platform::check_disk_space(&path).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
}
