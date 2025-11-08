use assert_cmd::cargo::cargo_bin;
use std::fs;
use std::io::Write;
use std::process::Command;
use std::thread;
use tempfile::tempdir;

// Large-ish integration stress: move two distinct directories (100 files each) concurrently via separate processes.
// Validates per-source and per-destination locking and lack of cross-process interference.
#[test]
fn move_two_directories_concurrently() -> Result<(), Box<dyn std::error::Error>> {
    let td = tempdir()?;
    let base = fs::canonicalize(td.path())?; // canonical to avoid symlink ancestors
    let download_base = base.join("downloads");
    let completed_base = base.join("completed");
    fs::create_dir_all(&download_base)?;
    fs::create_dir_all(&completed_base)?;

    // Two source dirs
    let dir_a = download_base.join("batch_a");
    let dir_b = download_base.join("batch_b");
    fs::create_dir_all(&dir_a)?;
    fs::create_dir_all(&dir_b)?;

    // Populate each with 100 small files (fsync to reduce flakiness)
    for i in 0..100 {
        let pa = dir_a.join(format!("a_{i:03}.dat"));
        let pb = dir_b.join(format!("b_{i:03}.dat"));
        for (p, label) in [(pa, 'A'), (pb, 'B')] {
            let mut f = fs::File::create(&p)?;
            write!(f, "{label}:{i}")?;
            f.sync_all()?;
        }
    }

    // Shared XML config so both processes use same bases
    let cfg_path = base.join("config.xml");
    let xml = format!(
        r#"<config>
  <download_base>{}</download_base>
  <completed_base>{}</completed_base>
  <log_level>normal</log_level>
  <preserve_metadata>false</preserve_metadata>
</config>"#,
        download_base.display(),
        completed_base.display()
    );
    fs::write(&cfg_path, xml)?;

    let bin = cargo_bin!("aria_move");

    // Spawn both processes concurrently.
    let cfg_a = cfg_path.clone();
    let cfg_b = cfg_path.clone();
    let dir_a_clone = dir_a.clone();
    let dir_b_clone = dir_b.clone();

    let handle_a = thread::spawn(move || {
        Command::new(&bin)
            .env("ARIA_MOVE_CONFIG", &cfg_a)
            .arg(&dir_a_clone) // pass directory path directly
            .output()
    });
    let handle_b = thread::spawn(move || {
        Command::new(&bin)
            .env("ARIA_MOVE_CONFIG", &cfg_b)
            .arg(&dir_b_clone)
            .output()
    });

    let out_a = handle_a.join().expect("join a")?;
    let out_b = handle_b.join().expect("join b")?;

    if !out_a.status.success() {
        eprintln!("Process A stderr:\n{}", String::from_utf8_lossy(&out_a.stderr));
        panic!("Process A failed");
    }
    if !out_b.status.success() {
        eprintln!("Process B stderr:\n{}", String::from_utf8_lossy(&out_b.stderr));
        panic!("Process B failed");
    }

    // Verify directories moved and sources removed.
    assert!(!dir_a.exists(), "dir_a should be removed after move");
    assert!(!dir_b.exists(), "dir_b should be removed after move");

    let dest_a = completed_base.join("batch_a");
    let dest_b = completed_base.join("batch_b");
    assert!(dest_a.exists(), "dest_a must exist");
    assert!(dest_b.exists(), "dest_b must exist");

    // Verify file counts and basic content spot-checks.
    let count_a = fs::read_dir(&dest_a)?.count();
    let count_b = fs::read_dir(&dest_b)?.count();
    assert_eq!(count_a, 100, "expected 100 files in dest_a (got {count_a})");
    assert_eq!(count_b, 100, "expected 100 files in dest_b (got {count_b})");

    // Spot-check a few files.
    for check in [0usize, 50, 99] {
        let fa = dest_a.join(format!("a_{check:03}.dat"));
        let fb = dest_b.join(format!("b_{check:03}.dat"));
        let ca = fs::read_to_string(&fa)?;
        let cb = fs::read_to_string(&fb)?;
        assert!(ca.starts_with("A:"), "unexpected content in {fa:?}");
        assert!(cb.starts_with("B:"), "unexpected content in {fb:?}");
    }

    Ok(())
}
