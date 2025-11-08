use std::fs;
use std::time::{Duration, SystemTime};
use tempfile::tempdir;

use aria_move::{Config};
use aria_move::fs_ops::resolve_source_path;

fn cfg_with(download: &std::path::Path) -> Config {
    let mut c = Config::default();
    c.download_base = download.to_path_buf();
    c.recent_window = Duration::from_secs(300);
    c
}

#[test]
fn provided_path_file_ok() {
    let td = tempdir().unwrap();
    let f = td.path().join("file.txt");
    fs::write(&f, b"x").unwrap();
    let cfg = cfg_with(td.path());
    let got = resolve_source_path(&cfg, Some(&f)).unwrap();
    assert_eq!(got, f);
}

#[test]
fn provided_path_dir_rejected() {
    let td = tempdir().unwrap();
    let d = td.path().join("d");
    fs::create_dir_all(&d).unwrap();
    let cfg = cfg_with(td.path());
    let err = resolve_source_path(&cfg, Some(&d)).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("not a regular file"));
}

#[test]
fn picks_newest_within_window() {
    let td = tempdir().unwrap();
    let d = td.path().join("base");
    fs::create_dir_all(&d).unwrap();
    let old = d.join("old.txt");
    let new = d.join("new.txt");
    fs::write(&old, b"a").unwrap();
    fs::write(&new, b"b").unwrap();
    // Make old older
    let past = SystemTime::now() - Duration::from_secs(3600);
    filetime::set_file_mtime(&old, filetime::FileTime::from_system_time(past)).unwrap();

    let mut cfg = Config::default();
    cfg.download_base = d.clone();
    cfg.recent_window = Duration::from_secs(60*5);

    let got = resolve_source_path(&cfg, None).unwrap();
    assert_eq!(got, new);
}

#[test]
fn returns_error_when_none_recent() {
    let td = tempdir().unwrap();
    let d = td.path().join("base");
    fs::create_dir_all(&d).unwrap();
    let a = d.join("a.txt");
    let b = d.join("b.txt");
    fs::write(&a, b"a").unwrap();
    fs::write(&b, b"b").unwrap();
    // both very old
    let past = SystemTime::now() - Duration::from_secs(86_400*10);
    let ft = filetime::FileTime::from_system_time(past);
    filetime::set_file_mtime(&a, ft).unwrap();
    filetime::set_file_mtime(&b, ft).unwrap();

    let mut cfg = Config::default();
    cfg.download_base = d.clone();
    cfg.recent_window = Duration::from_secs(1); // strict recent -> none recent

    // Should now fail instead of falling back
    let err = resolve_source_path(&cfg, None).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("No file found under base"));
}

#[test]
fn ignores_deny_suffixes() {
    let td = tempdir().unwrap();
    let d = td.path().join("base");
    fs::create_dir_all(&d).unwrap();
    let tmp = d.join("file.tmp");
    let real = d.join("real.bin");
    fs::write(&tmp, b"x").unwrap();
    fs::write(&real, b"y").unwrap();

    let mut cfg = Config::default();
    cfg.download_base = d.clone();
    cfg.recent_window = Duration::from_secs(3600);

    let got = resolve_source_path(&cfg, None).unwrap();
    assert_eq!(got, real);
}
