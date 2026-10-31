use anyhow::{Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use tracing::debug;

use crate::config::types::Config;
use crate::utils::ensure_not_base;

use super::dir_move::move_dir;
use super::file_move::move_file;

/// Top-level dispatcher for moving a single path (file or directory).
/// - Ensures `src` is not the configured download base.
/// - Dispatches on symlink-aware metadata; movers revalidate under their locks.
/// - Delegates to file or directory mover and returns the final destination path.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;

    // First use symlink_metadata to detect and reject symlinks explicitly.
    let lmeta = fs::symlink_metadata(src).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            crate::errors::io_context(src, e)
                .context(format!("Source does not exist: {}", src.display()))
        } else {
            crate::errors::io_context(src, e)
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

/// Shared checks for direct file/dir APIs as well as the dispatcher.
pub(super) fn validate_source(config: &Config, src: &Path, directory: bool) -> Result<()> {
    crate::shutdown::check()?;
    ensure_not_base(&config.download_base, src)?;
    crate::config::paths::reject_symlinks(src)?;
    crate::config::paths::reject_symlinks(&config.completed_base)?;
    let metadata = fs::symlink_metadata(src).map_err(|e| crate::errors::io_context(src, e))?;
    if metadata.is_dir() != directory || (!metadata.is_file() && !metadata.is_dir()) {
        bail!("Source has unexpected type: {}", src.display());
    }
    let source = crate::config::paths::normalize_missing(src)?;
    let destination = crate::config::paths::normalize_missing(&config.completed_base)?;
    if directory && (destination.starts_with(&source) || source.starts_with(&destination)) {
        bail!("Source and destination directories must be disjoint");
    }
    Ok(())
}
