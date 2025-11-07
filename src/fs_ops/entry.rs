use anyhow::{anyhow, bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::config::types::Config;
use crate::utils::ensure_not_base;

use super::dir_move::move_dir;
use super::file_move::move_file;

/// Top-level dispatcher for moving a single path (file or directory).
/// - Ensures `src` is not the configured download base.
/// - Stats once and branches based on the file type (avoids double syscalls).
/// - Delegates to file or directory mover and returns the final destination path.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;

    // First use symlink_metadata to detect and reject symlinks explicitly.
    let lmeta = fs::symlink_metadata(src).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            anyhow!("Source does not exist: {}", src.display())
        } else {
            e.into()
        }
    })?;

    let ftype = lmeta.file_type();
    if ftype.is_symlink() {
        bail!("Refusing to move symlink: {}", src.display());
    }

    // For regular files/dirs, a second metadata call isn't strictly necessary, but
    // keep using the symlink-aware result to branch without following links.
    debug!(path = %src.display(), is_file = ftype.is_file(), is_dir = ftype.is_dir(), "dispatch move_entry");

    if ftype.is_file() {
        move_file(config, src)
    } else if ftype.is_dir() {
        move_dir(config, src)
    } else {
        bail!(
            "Source path is neither a regular file nor a directory: {}",
            src.display()
        )
    }
}
