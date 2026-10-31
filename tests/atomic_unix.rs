#[cfg(unix)]
mod tests {
    use aria_move::fs_ops::{MoveOutcome, try_atomic_move};
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn rename_across_dirs_same_fs_persists() {
        let td = tempdir().unwrap();
        let a = td.path().join("a");
        let b = td.path().join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let src = a.join("file.txt");
        let mut f = fs::File::create(&src).unwrap();
        writeln!(f, "hello").unwrap();
        f.sync_all().unwrap();

        let dst = b.join("file.txt");
        let out = try_atomic_move(&src, &dst).unwrap();
        assert_eq!(out, MoveOutcome::Renamed);
        assert!(!src.exists(), "source should be gone after rename");
        let contents = fs::read_to_string(&dst).unwrap();
        assert!(contents.contains("hello"));
    }

    #[test]
    fn rename_over_existing_is_refused() {
        let td = tempdir().unwrap();
        let dir = td.path().join("d");
        fs::create_dir_all(&dir).unwrap();
        let src = dir.join("file.src.txt");
        fs::write(&src, "from-src").unwrap();
        let dst = dir.join("file.txt");
        fs::write(&dst, "old").unwrap();
        assert!(try_atomic_move(&src, &dst).is_err());
        assert_eq!(fs::read_to_string(&src).unwrap(), "from-src");
        assert_eq!(fs::read_to_string(&dst).unwrap(), "old");
    }
}
