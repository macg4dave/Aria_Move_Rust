use aria_move::{Config, fs_ops};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;
use walkdir::WalkDir;

fn mk_cfg(download: &Path, completed: &Path) -> Config {
    Config {
        download_base: download.to_path_buf(),
        completed_base: completed.to_path_buf(),
        preserve_metadata: false,
        dry_run: false,
        ..Config::default()
    }
}

fn build_tree(root: &Path) -> Vec<PathBuf> {
    let mut rel_files = Vec::new();
    // Deep-ish hierarchy with hidden, spaced, and nested names
    let layout = [
        "a.txt",
        "sub1/b.log",
        "sub1/sub2/c.bin",
        "sub1/sub 2/d.dat",
        ".hidden/e.cfg",
        "sub1/.hidden_inner/f.txt",
    ];
    for rel in layout.iter() {
        let p = root.join(rel);
        if let Some(parent) = p.parent() { fs::create_dir_all(parent).unwrap(); }
        fs::write(&p, rel.as_bytes()).unwrap();
        rel_files.push(PathBuf::from(rel));
    }
    rel_files
}

fn collect_relative_files(root: &Path) -> HashSet<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(root).unwrap().to_path_buf())
        .collect()
}

#[test]
fn move_dir_copy_fallback_preserves_all_files() -> Result<(), Box<dyn std::error::Error>> {
    // Force copy path (no atomic rename) to exercise copy branch thoroughly.
    unsafe { std::env::set_var("ARIA_MOVE_FORCE_DIR_COPY", "1"); }

    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path());

    let src_dir = download.path().join("complex_dir");
    fs::create_dir_all(&src_dir)?;
    let expected = build_tree(&src_dir);

    let before_set: HashSet<_> = expected.into_iter().collect();

    let dest = fs_ops::move_dir(&cfg, &src_dir)?;
    assert!(!src_dir.exists(), "source directory should be removed");
    assert!(dest.exists(), "destination directory should exist");

    let after_set = collect_relative_files(&dest);
    assert_eq!(before_set, after_set, "mismatch in copied file set: before={:?} after={:?}", before_set, after_set);

    // Also verify file contents match their relative names we wrote.
    for rel in &after_set {
        let contents = fs::read(dest.join(rel))?;
        assert_eq!(contents, rel.to_string_lossy().as_bytes(), "content mismatch for {:?}", rel);
    }

    unsafe { std::env::remove_var("ARIA_MOVE_FORCE_DIR_COPY"); }
    Ok(())
}
