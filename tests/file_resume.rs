use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

fn mk_cfg(download: &Path, completed: &Path) -> aria_move::Config {
    aria_move::Config {
        download_base: download.to_path_buf(),
        completed_base: completed.to_path_buf(),
        ..aria_move::Config::default()
    }
}

// Use the public util::resume_temp_path instead of duplicating hashing logic.
fn test_resume_temp_path(dest: &Path) -> std::path::PathBuf {
    aria_move::fs_ops::resume_temp_path(dest)
}

#[test]
fn resumes_partial_file_copy_and_finalizes() -> Result<(), Box<dyn std::error::Error>> {
    let download = tempdir()?;
    let completed = tempdir()?;
    let cfg = mk_cfg(download.path(), completed.path());

    let src = download.path().join("big.dat");
    let content = vec![42u8; 1024 * 1024 + 123]; // >1MiB
    fs::write(&src, &content)?;

    // Compute intended destination and temp path
    let dest = completed.path().join("big.dat");
    let tmp = test_resume_temp_path(&dest);

    // Pre-create partial temp file with first half of content
    if let Some(parent) = tmp.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(&tmp)?;
    let half = content.len() / 2;
    f.write_all(&content[..half])?;
    f.sync_all()?;

    // Invoke file move; it should resume and finalize rename
    let final_dest = aria_move::fs_ops::move_file(&cfg, &src)?;
    assert_eq!(final_dest, dest);

    // Source removed, destination exists and has full content
    assert!(!src.exists());
    let got = fs::read(&dest)?;
    assert_eq!(got, content);

    Ok(())
}
