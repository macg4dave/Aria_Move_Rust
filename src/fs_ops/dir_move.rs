//! Directory move implementation.
//! Strategy:
//! - Try atomic rename of the whole directory first (fast path).
//! - On failure (e.g., EXDEV), pre-check disk space, then copy the tree and remove the source.
//! Concurrency:
//! - Per-source move lock to avoid concurrent claims on the same source.
//! - Per-destination-base lock to serialize finalization into the completed_base.

use anyhow::{anyhow, bail, Context, Result};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::info;
use walkdir::WalkDir;

use crate::config::types::Config;
use crate::shutdown;
use crate::utils::{ensure_not_base, file_is_mutable};

use super::lock::{acquire_dir_lock, acquire_move_lock, io_error_with_help};
use super::space;

/// Move directory contents into completed_base/<src_dir_name>.
/// - Returns the final destination directory path on success.
/// - Dry-run prints intent and returns the target path.
pub fn move_dir(config: &Config, src_dir: &Path) -> Result<PathBuf> {
    if shutdown::is_requested() {
        bail!("shutdown requested");
    }

    // Serialize operations on this source to avoid double-processing.
    let _src_lock = acquire_move_lock(src_dir)?;
    ensure_not_base(&config.download_base, src_dir)?;

    // Compute the target path under completed_base.
    let src_name = src_dir
        .file_name()
        .ok_or_else(|| anyhow!("Source directory missing name: {}", src_dir.display()))?;
    let target = config.completed_base.join(src_name);

    if config.dry_run {
        info!(src = %src_dir.display(), dest = %target.display(), "dry-run: would move directory");
        return Ok(target);
    }

    // Serialize moves that finalize into the same completed_base to avoid races.
    let _dst_lock = acquire_dir_lock(&config.completed_base)
        .with_context(|| format!("acquire lock for '{}'", config.completed_base.display()))?;

    // Fast path: same-filesystem atomic directory rename.
    if fs::rename(src_dir, &target).is_ok() {
        info!(src = %src_dir.display(), dest = %target.display(), "Renamed directory atomically");
        return Ok(target);
    }

    // Cross-filesystem or other rename failures: fallback to copy.
    // Before copying, estimate total size and ensure destination has enough free space.
    let total_bytes = total_bytes_in_tree(src_dir);
    // Best-effort check; if statting sizes failed we still proceed, but enforce if we have a number.
    if let Some(required) = total_bytes {
        space::ensure_space_for_copy(&config.completed_base, required).with_context(|| {
            format!(
                "insufficient free space to copy '{}' (~{}) into '{}'",
                src_dir.display(),
                space::format_bytes(required),
                config.completed_base.display()
            )
        })?;
    }

    // 1) Create directory structure under target.
    WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_dir())
        .try_for_each(|d| -> Result<()> {
            if let Ok(rel) = d.path().strip_prefix(src_dir) {
                let new_dir = target.join(rel);
                fs::create_dir_all(&new_dir)
                    .map_err(io_error_with_help("create directory", &new_dir))?;
            }
            Ok(())
        })?;

    // 2) Collect files and copy them in parallel.
    let files: Vec<_> = WalkDir::new(src_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();

    files.par_iter().try_for_each(|path| -> Result<()> {
        // Skip files that appear to be in use to avoid partial copies.
        if file_is_mutable(path)? {
            return Err(anyhow!(
                "File '{}' seems in-use; aborting directory move",
                path.display()
            ));
        }

        let rel = path.strip_prefix(src_dir)?;
        let dst = target.join(rel);

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent).map_err(io_error_with_help("create directory", parent))?;
        }

        fs::copy(path, &dst).map_err(io_error_with_help("copy file to destination", &dst))?;
        Ok(())
    })?;

    // 3) Remove the original tree after successful copy.
    fs::remove_dir_all(src_dir).map_err(io_error_with_help("remove source directory", src_dir))?;

    info!(
        src = %src_dir.display(),
        dest = %target.display(),
        "Copied directory contents and removed source"
    );
    Ok(target)
}

/// Estimate total bytes of regular files under `root`.
/// Returns Some(bytes) on success, or None if any metadata read fails.
fn total_bytes_in_tree(root: &Path) -> Option<u64> {
    let mut total: u64 = 0;
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        if entry.file_type().is_file() {
            match entry.metadata() {
                Ok(m) => total = total.saturating_add(m.len()),
                Err(_) => return None, // give up on precise check; we'll proceed without enforcing
            }
        }
    }
    Some(total)
}
