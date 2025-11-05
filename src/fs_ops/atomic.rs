use anyhow::Result;
use std::fs;
use std::fs::OpenOptions;
use std::path::Path;

use super::helpers::io_error_with_help;

/// Try an atomic rename move. On Windows, remove pre-existing dest first.
pub(super) fn try_atomic_move(src: &Path, dest: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        if dest.exists() {
            fs::remove_file(dest).map_err(io_error_with_help(
                "remove existing destination before rename",
                dest,
            ))?;
        }
    }
    fs::rename(src, dest).map_err(io_error_with_help("rename source to destination", dest))?;

    if let Some(dest_dir) = dest.parent() {
        let dirf = OpenOptions::new()
            .read(true)
            .open(dest_dir)
            .map_err(io_error_with_help(
                "open destination directory for sync",
                dest_dir,
            ))?;
        dirf.sync_all()
            .map_err(io_error_with_help("fsync destination directory", dest_dir))?;
    }
    Ok(())
}
