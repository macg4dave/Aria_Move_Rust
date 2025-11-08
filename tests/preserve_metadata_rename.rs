#[cfg(unix)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, SystemTime};
    use tempfile::tempdir;
    use aria_move::{Config};
    use aria_move::fs_ops::move_file;

    #[test]
    fn preserve_metadata_applies_after_rename() {
        let td = tempdir().unwrap();
        let download = td.path().join("incoming");
        let completed = td.path().join("completed");
        fs::create_dir_all(&download).unwrap();
        fs::create_dir_all(&completed).unwrap();

        // Create source file with specific mode and mtime
        let src = download.join("meta.txt");
        fs::write(&src, "data").unwrap();
        let mut perms = fs::metadata(&src).unwrap().permissions();
        perms.set_mode(0o640);
        fs::set_permissions(&src, perms).unwrap();

        // Set mtime to a fixed time in the past
        let past = SystemTime::now() - Duration::from_secs(3600);
        let ft = filetime::FileTime::from_system_time(past);
        filetime::set_file_times(&src, ft, ft).unwrap();

        // Run move with preserve_metadata=true
        let mut cfg = Config::new(&download, &completed, Duration::from_secs(60));
        cfg.preserve_metadata = true;
        let dest = move_file(&cfg, &src).unwrap();

        // Verify mode and mtime preserved
        let meta = fs::metadata(&dest).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o640);
        let mtime = filetime::FileTime::from_last_modification_time(&meta);
        // Allow small differences, but it should be close to 'past' (within a few seconds)
    let past_secs = ft.seconds();
    let mt_secs = mtime.seconds();
    assert!(mt_secs.abs_diff(past_secs) <= 5, "mtime not preserved sufficiently: got {} expected ~{}", mt_secs, past_secs);

        // Also verify atime preserved approximately
        let atime = filetime::FileTime::from_last_access_time(&meta);
    let at_secs = atime.seconds();
    assert!(at_secs.abs_diff(past_secs) <= 5, "atime not preserved sufficiently: got {} expected ~{}", at_secs, past_secs);
    }
}
