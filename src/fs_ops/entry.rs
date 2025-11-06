use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

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

    let meta = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;

    if meta.is_file() {
        move_file(config, src)
    } else if meta.is_dir() {
        move_dir(config, src)
    } else {
        bail!(
            "Source path is neither a regular file nor a directory: {}",
            src.display()
        )
    }
}
