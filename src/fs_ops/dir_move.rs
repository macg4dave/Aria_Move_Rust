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
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::config::types::Config;
use crate::shutdown;
use crate::utils::{ensure_not_base, file_is_mutable};

use super::lock::{acquire_dir_lock, acquire_move_lock};
use super::io_error_with_help;
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
    let mut target = config.completed_base.join(src_name);
    if target.exists() {
        // Mirror file move behavior: choose a unique destination directory name.
        target = crate::utils::unique_destination(&target);
    }

    if config.dry_run {
        info!(src = %src_dir.display(), dest = %target.display(), "dry-run: would move directory");
        return Ok(target);
    }

    // Serialize moves that finalize into the same completed_base to avoid races.
    let _dst_lock = acquire_dir_lock(&config.completed_base)
        .with_context(|| format!("acquire lock for '{}'", config.completed_base.display()))?;

    // Fast path: same-filesystem atomic directory rename.
    // Optional pre-detect of cross-device (Unix) to skip a failing rename.
    let mut did_rename = false;

    // In tests, allow forcing the copy fallback to exercise that path.
    #[cfg(test)]
    let force_copy = std::env::var("ARIA_MOVE_FORCE_DIR_COPY").ok().as_deref() == Some("1");
    #[cfg(not(test))]
    let force_copy = false;

    #[cfg(unix)]
    let cross_device = if let (Some(src_parent), Some(dst_parent)) = (src_dir.parent(), target.parent()) {
        use std::os::unix::fs::MetadataExt;
        if let (Ok(s_meta), Ok(d_meta)) = (fs::metadata(src_parent), fs::metadata(dst_parent)) {
            s_meta.dev() != d_meta.dev()
        } else { false }
    } else { false };
    #[cfg(not(unix))]
    let cross_device = false;

    if !force_copy && !cross_device {
        match fs::rename(src_dir, &target) {
            Ok(()) => {
                info!(src = %src_dir.display(), dest = %target.display(), "Renamed directory atomically");
                // Best-effort fsync of destination parent (and source parent if different) on Unix.
                #[cfg(unix)]
                {
                    if let Some(dst_parent) = target.parent() {
                        if let Err(e) = super::util::fsync_dir(dst_parent) {
                            warn!(error = %e, dir = %dst_parent.display(), "best-effort fsync(dst_parent) failed");
                        }
                    }
                    if let (Some(sp), Some(dp)) = (src_dir.parent(), target.parent()) {
                        if sp != dp {
                            if let Err(e) = super::util::fsync_dir(sp) {
                                warn!(error = %e, dir = %sp.display(), "best-effort fsync(src_parent) failed");
                            }
                        }
                    }
                }
                did_rename = true;
            }
            Err(e) => {
                // Proceed to copy fallback; log a short hint.
                let hint: &str = if let Some(code) = e.raw_os_error() {
                    #[cfg(unix)]
                    {
                        if code == libc::EXDEV { "cross-filesystem; will copy instead" }
                        else if code == libc::EACCES || code == libc::EPERM { "permission denied; check destination perms" }
                        else { "falling back to copy" }
                    }
                    #[cfg(not(unix))]
                    {
                        if e.kind() == std::io::ErrorKind::PermissionDenied { "permission denied; check destination perms" } else { "falling back to copy" }
                    }
                } else { "falling back to copy" };
                warn!(error = %e, hint, "Atomic directory rename failed, using copy fallback");
            }
        }
    }
    if did_rename {
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

    let copy_result: Result<()> = files.par_iter().try_for_each(|path| -> Result<()> {
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

        // Copy file data
        fs::copy(path, &dst).map_err(io_error_with_help("copy file to destination", &dst))?;
        // Metadata preservation; apply full or permissions-only per flags (best-effort)
        if config.preserve_metadata || config.preserve_permissions {
            if let Ok(src_meta) = fs::metadata(path) {
                if config.preserve_metadata {
                    let _ = super::metadata::preserve_metadata(&dst, &src_meta);
                    let _ = super::metadata::preserve_xattrs(path, &dst);
                } else {
                    let _ = super::metadata::preserve_permissions_only(&dst, &src_meta);
                }
            }
        }
        Ok(())
    });
    if let Err(e) = copy_result {
        // Partial failure cleanup: remove target subtree to avoid half-copied results.
        let _ = fs::remove_dir_all(&target);
        return Err(e);
    }

    // 3) Remove the original tree after successful copy.
    fs::remove_dir_all(src_dir).map_err(io_error_with_help("remove source directory", src_dir))?;

    // Best-effort fsync of the destination directory to persist entries.
    #[cfg(unix)]
    if let Err(e) = super::util::fsync_dir(&target) {
        warn!(error = %e, dir = %target.display(), "best-effort fsync(target) failed");
    }

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
