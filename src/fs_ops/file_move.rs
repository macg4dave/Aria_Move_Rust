//! File move implementation:
//! - Fast path: atomic rename into completed_base
//! - Fallback: safe copy -> fsync -> atomic rename, then remove source
//! - Optional: preserve src permissions/timestamps on destination
//!   Concurrency:
//! - Per-source lock to prevent double-processing of the same item
//! - Per-destination-base lock to serialize finalization inside completed_base

use anyhow::{Context, Result, anyhow};
use std::fs::{self};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::config::types::Config;
use crate::errors::AriaMoveError;
use crate::platform::check_disk_space;
use crate::shutdown;
use crate::utils::{ensure_not_base, stable_file_probe, unique_destination};

use super::atomic::{MoveOutcome, try_atomic_move};
use super::copy::safe_copy_and_rename_with_metadata;
use super::io_error_with_help;
use super::lock::{acquire_dir_lock, acquire_move_lock};
use super::metadata;

/// Move a single file into `completed_base`.
/// Returns the final destination path.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    // Honor shutdown request early.
    if shutdown::is_requested() {
        return Err(AriaMoveError::Interrupted.into());
    }

    // Serialize on this source and ensure it's stable (size/mtime unchanged briefly).
    // Optional: allow disabling locks for environments where directory flock is denied.
    let disable_locks = config.disable_locks
        || std::env::var("ARIA_MOVE_DISABLE_LOCKS").ok().as_deref() == Some("1");
    let _move_lock: Option<super::lock::DirLock> = if disable_locks {
        debug!(src = %src.display(), "locks disabled via config or ARIA_MOVE_DISABLE_LOCKS=1 (source)");
        None
    } else {
        match acquire_move_lock(src) {
            Ok(l) => Some(l),
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    debug!(error = %e, src = %src.display(), "acquire_move_lock permission denied; proceeding without lock (diagnostic)");
                    None
                } else {
                    return Err(e.into());
                }
            }
        }
    };
    ensure_not_base(&config.download_base, src)?;
    stable_file_probe(src, Duration::from_millis(200), 3)?;

    // Compute final destination path (deduplicate name if needed).
    let dest_dir = &config.completed_base;

    if !config.dry_run {
        if let Err(e) = fs::create_dir_all(dest_dir) {
            if e.kind() == io::ErrorKind::PermissionDenied {
                debug!(error = %e, dest = %dest_dir.display(), "create_dir_all permission denied");
            }
            return Err(io_error_with_help("create destination directory", dest_dir)(e));
        }
    } else {
        // Dry-run: keep a light permission check to surface obvious issues without writing.
        info!(action = "mkdir -p", path = %dest_dir.display(), "dry-run");
        if let Some(parent) = dest_dir.parent()
            && (!parent.exists() || parent.metadata()?.permissions().readonly())
        {
            return Err(AriaMoveError::PermissionDenied {
                path: dest_dir.to_path_buf(),
                context: "dry-run parent missing or readonly".into(),
            }
            .into());
        }
    }

    if config.dry_run {
        // Dry-run: compute and return intended destination without taking locks.
        let file_name = src
            .file_name()
            .ok_or_else(|| anyhow!("Source file missing a file name: {}", src.display()))?;
        let mut dest = dest_dir.join(file_name);
        if dest.exists() {
            dest = unique_destination(&dest);
        }
        info!(src = %src.display(), dest = %dest.display(), "dry-run: would move file");
        return Ok(dest);
    }

    // Serialize finalization into completed_base to avoid races on destination naming and final rename.
    let _dir_lock: Option<super::lock::DirLock> = if disable_locks {
        debug!(dest = %dest_dir.display(), "locks disabled via config or ARIA_MOVE_DISABLE_LOCKS=1 (dest)");
        None
    } else {
        match acquire_dir_lock(dest_dir) {
            Ok(l) => Some(l),
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    debug!(error = %e, dest = %dest_dir.display(), "acquire_dir_lock permission denied; proceeding without lock (diagnostic)");
                    None
                } else {
                    return Err(anyhow!("acquire lock for '{}': {}", dest_dir.display(), e));
                }
            }
        }
    };

    // Now decide final destination name while holding the directory lock.
    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow!("Source file missing a file name: {}", src.display()))?;
    let mut dest = dest_dir.join(file_name);
    if dest.exists() {
        dest = unique_destination(&dest);
    }

    // Capture source metadata BEFORE any rename (after rename, src path no longer exists).
    let meta_before = if config.preserve_metadata || config.preserve_permissions {
        Some(fs::metadata(src).with_context(|| format!("stat {}", src.display()))?)
    } else {
        None
    };

    // Fast path: atomic rename (same filesystem). May return CrossDevice prediction.
    match try_atomic_move(src, &dest) {
        Ok(MoveOutcome::Renamed) => {
            debug!(src = %src.display(), dest = %dest.display(), "Renamed file atomically");
            if let Some(meta) = meta_before.as_ref() {
                if config.preserve_metadata {
                    let _ = metadata::preserve_metadata(&dest, meta);
                    let _ = metadata::preserve_xattrs(src, &dest);
                } else if config.preserve_permissions {
                    let _ = metadata::preserve_permissions_only(&dest, meta);
                }
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

            warn!(error = %e, hint, src = %src.display(), dest = %dest.display(), "Atomic rename failed, using safe copy+rename");
        }
    }

    // Before copying across filesystems, ensure the destination has enough space.
    let src_size = match fs::metadata(src) {
        Ok(m) => m.len(),
        Err(e) => {
            if e.kind() == io::ErrorKind::PermissionDenied {
                debug!(error = %e, src = %src.display(), "metadata stat permission denied");
            }
            return Err(anyhow!("stat source {}: {}", src.display(), e));
        }
    };
    let available = match check_disk_space(dest_dir) {
        Ok(av) => av,
        Err(e) => {
            debug!(error = %e, dest = %dest_dir.display(), "disk space check failed");
            return Err(anyhow!("check disk space at {}: {}", dest_dir.display(), e));
        }
    };
    if available < src_size {
        return Err(AriaMoveError::InsufficientSpace {
            required: src_size as u128,
            available: available as u128,
            dest: dest_dir.to_path_buf(),
        }
        .into());
    }
    // Copy with or without metadata; permissions-only handled after file is at dest.
    safe_copy_and_rename_with_metadata(src, &dest, config.preserve_metadata)?;

    // Remove original after successful copy into place.
    match fs::remove_file(src) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::NotFound => { /* already gone; ignore */ }
        Err(e) => return Err(io_error_with_help("remove original file", src)(e)),
    }

    // Best-effort fsync of the source parent to persist the deletion on Unix.
    #[cfg(unix)]
    if let Some(src_parent) = src.parent()
        && let Err(e) = super::util::fsync_dir(src_parent)
    {
        warn!(error = %e, dir = %src_parent.display(), "best-effort fsync(src_parent after delete) failed");
    }

    // If only permissions (not full metadata) requested, apply now at dest
    if let Some(meta) = meta_before.as_ref()
        && !config.preserve_metadata
        && config.preserve_permissions
    {
        let _ = metadata::preserve_permissions_only(&dest, meta);
    }

    info!(src = %src.display(), dest = %dest.display(), "Copied file and removed source");
    Ok(dest)
}
