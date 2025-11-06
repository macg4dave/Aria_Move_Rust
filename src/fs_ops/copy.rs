//! Safe copy-and-rename helper.
//! Copies to a temp file in the destination directory, fsyncs, renames into place, and fsyncs the parent.

use anyhow::{anyhow, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::path::Path;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{io_copy, metadata, util};
use super::lock::io_error_with_help;

/// Copy src -> temp-in-dest-dir, fsync temp file, rename temp -> dest, fsync parent dir.
pub fn safe_copy_and_rename(src: &Path, dest: &Path) -> Result<()> {
    let dest_dir = dest
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Destination has no parent: {}", dest.display()))?;

    fs::create_dir_all(dest_dir)
        .map_err(io_error_with_help("create destination directory", dest_dir))?;

    let pid = process::id();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| anyhow!("time error: {}", e))?
        .as_millis();
    let tmp_name = format!(".aria_move.tmp.{}.{}", pid, now);
    let tmp_path = dest_dir.join(&tmp_name);

    // Stream-copy to avoid large memory usage and to ensure we write on the destination fs.
    io_copy::copy_streaming(src, &tmp_path).map_err(io_error_with_help("copy to temporary file", &tmp_path))?;

    let f = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&tmp_path)
        .map_err(io_error_with_help("open temporary file for sync", &tmp_path))?;
    f.sync_all()
        .map_err(io_error_with_help("fsync temporary file", &tmp_path))?;

    #[cfg(windows)]
    {
        if dest.exists() {
            fs::remove_file(dest).map_err(io_error_with_help(
                "remove existing destination before rename",
                dest,
            ))?;
        }
    }

    fs::rename(&tmp_path, dest).map_err(io_error_with_help(
        "rename temporary file to destination",
        dest,
    ))?;

    let dirf = OpenOptions::new()
        .read(true)
        .open(dest_dir)
        .map_err(io_error_with_help(
            "open destination directory for sync",
            dest_dir,
        ))?;
    dirf.sync_all()
        .map_err(io_error_with_help("fsync destination directory", dest_dir))?;

    Ok(())
}

/// Same as safe_copy_and_rename, but optionally preserves src permissions and mtime on dest.
pub fn safe_copy_and_rename_with_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    safe_copy_and_rename(src, dest)?;
    if preserve {
        let meta = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;
        metadata::preserve_metadata(dest, &meta)
            .with_context(|| format!("preserve metadata for {}", dest.display()))?;
    }
    Ok(())
}
