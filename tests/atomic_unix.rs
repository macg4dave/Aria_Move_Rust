#[cfg(unix)]
mod tests {
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;
    use aria_move::fs_ops::{try_atomic_move, MoveOutcome};

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
    fn rename_over_existing_overwrites() {
        let td = tempdir().unwrap();
        let dir = td.path().join("d");
        fs::create_dir_all(&dir).unwrap();
    let src = dir.join("file.src.txt");
        fs::write(&src, "from-src").unwrap();
    let dst = dir.join("file.txt");
        fs::write(&dst, "old").unwrap();
        // On Unix, rename overwrites; function should succeed and dst reflect new content
    let out = try_atomic_move(&src, &dst).unwrap();
    assert_eq!(out, MoveOutcome::Renamed);
        assert!(!src.exists());
        let s = fs::read_to_string(&dst).unwrap();
        assert_eq!(s, "from-src");
    }
}
