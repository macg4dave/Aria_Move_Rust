use assert_fs::TempDir;
use aria_move::config::{validate_and_normalize, Config};
use std::fs;

#[test]
fn completed_base_is_created_when_missing() {
    let td = TempDir::new().unwrap();
    let root = dunce::canonicalize(td.path()).unwrap();
    let download = root.join("incoming");
    fs::create_dir_all(&download).unwrap();
    let completed = root.join("completed_missing");
    assert!(!completed.exists());

    let mut cfg = Config::new(&download, &completed);
    validate_and_normalize(&mut cfg).expect("validation succeeds creating completed_base");
    assert!(completed.exists(), "completed_base should be created");
}

#[test]
fn download_and_completed_created_when_missing() {
    let td = TempDir::new().unwrap();
    let root = dunce::canonicalize(td.path()).unwrap();
    let download = root.join("incoming_missing");
    let completed = root.join("completed_missing");
    let mut cfg = Config::new(&download, &completed);
    validate_and_normalize(&mut cfg).expect("validation succeeds creating both bases");
    assert!(download.exists(), "download_base should be created");
    assert!(completed.exists(), "completed_base should be created");
}

#[test]
fn disallow_equal_paths() {
    let td = TempDir::new().unwrap();
    let root = dunce::canonicalize(td.path()).unwrap();
    let base = root.join("same");
    fs::create_dir_all(&base).unwrap();
    let mut cfg = Config::new(&base, &base);
    let err = validate_and_normalize(&mut cfg).unwrap_err();
    assert!(format!("{err}").contains("resolve to the same"));
}

#[test]
fn disallow_nested_download_inside_completed() {
    let td = TempDir::new().unwrap();
    let root = dunce::canonicalize(td.path()).unwrap();
    let completed = root.join("completed");
    fs::create_dir_all(&completed).unwrap();
    let download = completed.join("incoming");
    fs::create_dir_all(&download).unwrap();
    let mut cfg = Config::new(&download, &completed);
    let err = validate_and_normalize(&mut cfg).unwrap_err();
    assert!(format!("{err}").contains("must not be inside completed_base"));
}

#[test]
fn disallow_nested_completed_inside_download() {
    let td = TempDir::new().unwrap();
    let root = dunce::canonicalize(td.path()).unwrap();
    let download = root.join("incoming");
    fs::create_dir_all(&download).unwrap();
    let completed = download.join("completed");
    fs::create_dir_all(&completed).unwrap();
    let mut cfg = Config::new(&download, &completed);
    let err = validate_and_normalize(&mut cfg).unwrap_err();
    assert!(format!("{err}").contains("must not be inside download_base"));
}

#[cfg(unix)]
#[test]
fn reject_symlink_ancestor() {
    use std::os::unix::fs as unix_fs;
    let td = TempDir::new().unwrap();
    let root = dunce::canonicalize(td.path()).unwrap();
    let real = root.join("real_root");
    fs::create_dir_all(&real).unwrap();
    let link = root.join("link_root");
    unix_fs::symlink(&real, &link).unwrap();
    // inside real root create subdirs
    let incoming = real.join("incoming");
    fs::create_dir_all(&incoming).unwrap();
    let completed = real.join("completed");
    fs::create_dir_all(&completed).unwrap();
    // point cfg at paths through the symlink
    let mut cfg = Config::new(link.join("incoming"), link.join("completed"));
    let res = validate_and_normalize(&mut cfg);
    assert!(res.is_err(), "expected rejection when a symlink is in an ancestor path");
}
