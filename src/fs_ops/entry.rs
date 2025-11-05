use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::utils::ensure_not_base;

use super::dir_move::move_dir;
use super::file_move::move_file;

/// Top-level move entry: file or directory.
pub fn move_entry(config: &Config, src: &Path) -> Result<PathBuf> {
    ensure_not_base(&config.download_base, src)?;
    if src.is_file() {
        move_file(config, src)
    } else if src.is_dir() {
        move_dir(config, src)
    } else {
        bail!(
            "Source path is neither file nor directory: {}",
            src.display()
        )
    }
}
