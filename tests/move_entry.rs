use aria_move::{Config, fs_ops};
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn mk_cfg(download: &Path, completed: &Path, preserve_metadata: bool, dry_run: bool) -> Config {
    Config {
        download_base: download.to_path_buf(),
        completed_base: completed.to_path_buf(),
        preserve_metadata,
        dry_run,
        ..Config::default()
    }
}

#[test]
fn move_entry_file_ok() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, false);
    let src = download.path().join("f.txt");
    let mut f = fs::File::create(&src)?;
    writeln!(f, "hello")?;
    f.sync_all()?;
    let dest = fs_ops::move_entry(&cfg, &src)?;
    assert!(!src.exists());
    assert!(dest.exists());
    assert_eq!(fs::read_to_string(dest)?, "hello\n");
    Ok(())
}

#[test]
fn move_entry_dir_ok() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), true, false);
    let src_dir = download.path().join("folder");
    fs::create_dir_all(&src_dir)?;
    let sub = src_dir.join("sub");
    fs::create_dir_all(&sub)?;
    fs::write(sub.join("a.txt"), "A")?;
    let dest = fs_ops::move_entry(&cfg, &src_dir)?;
    assert!(!src_dir.exists());
    assert!(dest.exists());
    assert_eq!(fs::read_to_string(dest.join("sub/a.txt"))?, "A");
    Ok(())
}

#[cfg(unix)]
#[test]
fn move_entry_symlink_refused() {
    use std::os::unix::fs::symlink;
    let download = tempdir().unwrap();
    let completed = tempdir().unwrap();
    let cfg = Config {
        download_base: download.path().to_path_buf(),
        completed_base: completed.path().to_path_buf(),
        ..Config::default()
    };
    let target = download.path().join("real.txt");
    fs::write(&target, "real").unwrap();
    let link = download.path().join("link.txt");
    symlink(&target, &link).unwrap();
    let err = fs_ops::move_entry(&cfg, &link).unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("Refusing to move symlink"),
        "expected symlink refusal, got: {msg}"
    );
}

#[cfg(unix)]
#[test]
fn move_entry_special_file_refused() {
    use std::os::unix::fs::FileTypeExt;
    use std::process::Command;
    let download = tempdir().unwrap();
    let completed = tempdir().unwrap();
    let cfg = Config {
        download_base: download.path().to_path_buf(),
        completed_base: completed.path().to_path_buf(),
        ..Config::default()
    };
    // Create a FIFO (named pipe)
    let fifo = download.path().join("mypipe");
    let status = Command::new("mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success(), "mkfifo should succeed");
    // Sanity: metadata identifies not regular file nor dir
    let ftype = fs::symlink_metadata(&fifo).unwrap().file_type();
    assert!(ftype.is_fifo());
    let err = fs_ops::move_entry(&cfg, &fifo).unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("neither a regular file nor a directory"),
        "expected special-file rejection, got: {msg}"
    );
}

#[test]
fn move_entry_missing_path_error() {
    let download = tempdir().unwrap();
    let completed = tempdir().unwrap();
    let cfg = Config {
        download_base: download.path().to_path_buf(),
        completed_base: completed.path().to_path_buf(),
        ..Config::default()
    };
    let missing = download.path().join("nope.bin");
    let err = fs_ops::move_entry(&cfg, &missing).unwrap_err();
    let msg = format!("{}", err);
    assert!(
        msg.contains("does not exist"),
        "expected not-exist context, got: {msg}"
    );
}

#[test]
fn move_entry_dry_run_leaves_source() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path(), false, true);
    let src = download.path().join("dry.txt");
    fs::write(&src, "dry")?;
    let dest = fs_ops::move_entry(&cfg, &src)?;
    assert!(src.exists());
    assert!(!dest.exists());
    Ok(())
}
