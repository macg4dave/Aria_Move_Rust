//! Directory move implementation.
//! Tries a rename first; if that fails, copies the tree (parallelized) and removes the source.

use anyhow::{anyhow, bail, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

use crate::config::types::Config;
use crate::shutdown;
use crate::utils::{ensure_not_base, file_is_mutable};

use super::lock::acquire_move_lock;
use super::lock::io_error_with_help;

/// Move directory contents into completed_base/<src_dir_name>.
pub fn move_dir(config: &Config, src_dir: &Path) -> Result<PathBuf> {
    if shutdown::is_requested() {
        bail!("shutdown requested");
    }

    let _move_lock = acquire_move_lock(src_dir)?;
    ensure_not_base(&config.download_base, src_dir)?;

    let src_name = src_dir
        .file_name()
        .ok_or_else(|| anyhow!("Source directory missing name: {}", src_dir.display()))?;
    let target = config.completed_base.join(src_name);

    if config.dry_run {
        info!(src = %src_dir.display(), dest = %target.display(), "dry-run: would move directory");
        return Ok(target);
    }

    if std::fs::rename(src_dir, &target).is_ok() {
        info!(src = %src_dir.display(), dest = %target.display(), "Renamed directory atomically");
        return Ok(target);
    }

    WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .try_for_each(|d| -> Result<()> {
            if let Ok(rel) = d.path().strip_prefix(src_dir) {
                let new_dir = target.join(rel);
                std::fs::create_dir_all(&new_dir)
                    .map_err(io_error_with_help("create directory", &new_dir))?;
            }
            Ok(())
        })?;

    let files: Vec<_> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    files.par_iter().try_for_each(|path| -> Result<()> {
        if file_is_mutable(path)? {
            return Err(anyhow!(
                "File '{}' seems in-use; aborting directory move",
                path.display()
            ));
        }
        let rel = path.strip_prefix(src_dir)?;
        let dst = target.join(rel);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)
                .map_err(io_error_with_help("create directory", parent))?;
        }
        std::fs::copy(path, &dst).map_err(io_error_with_help("copy file to destination", &dst))?;
        Ok(())
    })?;

    std::fs::remove_dir_all(src_dir)
        .map_err(io_error_with_help("remove source directory", src_dir))?;
    info!(src = %src_dir.display(), dest = %target.display(), "Copied directory contents and removed source");
    Ok(target)
}
