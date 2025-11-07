//! File move implementation:
//! - Fast path: atomic rename into completed_base
//! - Fallback: safe copy -> fsync -> atomic rename, then remove source
//! - Optional: preserve src permissions/timestamps on destination
//! Concurrency:
//! - Per-source lock to prevent double-processing of the same item
//! - Per-destination-base lock to serialize finalization inside completed_base

use anyhow::{anyhow, Context, Result};
use std::fs::{self};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

use crate::config::types::Config;
use crate::errors::AriaMoveError;
use crate::platform::check_disk_space;
use crate::shutdown;
use crate::utils::{ensure_not_base, stable_file_probe, unique_destination};

use super::atomic::{try_atomic_move, MoveOutcome};
use super::copy::safe_copy_and_rename_with_metadata;
use super::lock::{acquire_dir_lock, acquire_move_lock, io_error_with_help};
use super::metadata;

/// Move a single file into `completed_base`.
/// Returns the final destination path.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    // Honor shutdown request early.
    if shutdown::is_requested() {
        return Err(AriaMoveError::Interrupted.into());
    }

    // Serialize on this source and ensure it's stable (size/mtime unchanged briefly).
    let _move_lock = acquire_move_lock(src)?;
    ensure_not_base(&config.download_base, src)?;
    stable_file_probe(src, Duration::from_millis(200), 3)?;

    // Compute final destination path (deduplicate name if needed).
    let dest_dir = &config.completed_base;

    if !config.dry_run {
        fs::create_dir_all(dest_dir)
            .map_err(io_error_with_help("create destination directory", dest_dir))?;
    } else {
        // Dry-run: keep a light permission check to surface obvious issues without writing.
        info!(action = "mkdir -p", path = %dest_dir.display(), "dry-run");
        if let Some(parent) = dest_dir.parent() {
            if !(parent.exists() && !parent.metadata()?.permissions().readonly()) {
                return Err(AriaMoveError::PermissionDenied {
                    path: dest_dir.to_path_buf(),
                    context: "dry-run parent missing or readonly".into(),
                }
                .into());
            }
        }
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow!("Source file missing a file name: {}", src.display()))?;
    let mut dest = dest_dir.join(file_name);
    if dest.exists() {
        dest = unique_destination(&dest);
    }

    if config.dry_run {
        info!(src = %src.display(), dest = %dest.display(), "dry-run: would move file");
        return Ok(dest);
    }

    // Serialize finalization into completed_base to avoid races on the final rename.
    let _dir_lock = acquire_dir_lock(dest_dir)
        .with_context(|| format!("acquire lock for '{}'", dest_dir.display()))?;

    // Capture source metadata BEFORE any rename (after rename, src path no longer exists).
    let meta_before = if config.preserve_metadata {
        Some(fs::metadata(src).with_context(|| format!("stat {}", src.display()))?)
    } else {
        None
    };

    // Fast path: atomic rename (same filesystem). May return CrossDevice prediction.
    match try_atomic_move(src, &dest) {
        Ok(MoveOutcome::Renamed) => {
            info!(src = %src.display(), dest = %dest.display(), "Renamed file atomically");
            if let Some(meta) = meta_before.as_ref() {
                metadata::preserve_metadata(&dest, meta).ok();
            }
            return Ok(dest);
        }
        Ok(MoveOutcome::CrossDevice) => {
            info!(src = %src.display(), dest = %dest.display(), "Cross-device move detected; using copy fallback");
        }
        Err(e) => {
            // Compute a short hint for logs; still proceed to copy fallback.
            let hint: &str = if let Some(ioe) = e.downcast_ref::<io::Error>() {
                if super::util::is_cross_device(ioe) {
                    "cross-filesystem; will copy instead"
                } else if ioe.kind() == io::ErrorKind::PermissionDenied {
                    "permission denied; check destination perms"
                } else {
                    "falling back to copy"
                }
            } else {
                "falling back to copy"
            };

            warn!(error = %e, hint, "Atomic rename failed, using safe copy+rename");
        }
    }

    // Before copying across filesystems, ensure the destination has enough space.
    let src_size = fs::metadata(src)
        .with_context(|| format!("stat source {}", src.display()))?
        .len();
    let available = check_disk_space(dest_dir)
        .with_context(|| format!("check disk space at {}", dest_dir.display()))?;
    if available < src_size as u64 {
        return Err(AriaMoveError::InsufficientSpace {
            required: src_size as u128,
            available: available as u128,
            dest: dest_dir.to_path_buf(),
        }
        .into());
    }
    safe_copy_and_rename_with_metadata(src, &dest, config.preserve_metadata)?;

    // Remove original after successful copy into place.
    fs::remove_file(src).map_err(io_error_with_help("remove original file", src))?;
    Ok(dest)
}
