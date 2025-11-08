use std::fs;
use std::path::Path;
// use std::time::Duration;
use tempfile::tempdir;

use aria_move::Config;
use aria_move::fs_ops::resolve_source_path;

fn cfg_with(download: &std::path::Path) -> Config {
    Config { download_base: download.to_path_buf(), ..Config::default() }
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
fn provided_path_dir_ok() {
    let td = tempdir().unwrap();
    let d = td.path().join("d");
    fs::create_dir_all(&d).unwrap();
    let cfg = cfg_with(td.path());
    let got = resolve_source_path(&cfg, Some(&d)).unwrap();
    assert_eq!(got, d);
}

#[test]
fn error_when_no_explicit_path_provided() {
    let td = tempdir().unwrap();
    let d = td.path().join("base");
    fs::create_dir_all(&d).unwrap();
    let cfg = Config { download_base: d.clone(), ..Config::default() };
    let err = resolve_source_path(&cfg, None).unwrap_err();
    let s = format!("{err}");
    assert!(s.contains("No file found under base"));
}

#[test]
fn bare_filename_falls_back_to_download_base() {
    let td = tempdir().unwrap();
    let base = td.path().join("base");
    fs::create_dir_all(&base).unwrap();
    let cfg = Config { download_base: base.clone(), ..Config::default() };

    // Create file under base, but provide only the filename
    let fname = "onlyname.txt";
    let full = base.join(fname);
    fs::write(&full, b"x").unwrap();

    let got = resolve_source_path(&cfg, Some(Path::new(fname))).unwrap();
    assert_eq!(got, full);
}
