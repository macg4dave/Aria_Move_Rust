//! File move implementation: atomic rename, fallback to safe copy+rename, optional metadata, locks.

use anyhow::{anyhow, Context, Result};
use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

use crate::config::types::Config;
use crate::errors::AriaMoveError;
use crate::shutdown;
use crate::utils::{ensure_not_base, stable_file_probe, unique_destination};

use super::atomic::try_atomic_move;
use super::copy::safe_copy_and_rename_with_metadata;
use super::lock::{acquire_dir_lock, acquire_move_lock, io_error_with_help};
use super::metadata;
use super::{claim, io_copy, space, util};
use crate::platform::check_disk_space;

/// Move a single file into `completed_base`.
/// Returns the final destination path.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    if shutdown::is_requested() {
        return Err(AriaMoveError::Interrupted.into());
    }
    // Acquire per-file move lock and verify readiness.
    let _move_lock = acquire_move_lock(src)?;
    ensure_not_base(&config.download_base, src)?;
    stable_file_probe(src, Duration::from_millis(200), 3)?;

    // Resolve destination path (unique if exists).
    let dest_dir = &config.completed_base;
    if !config.dry_run {
        fs::create_dir_all(dest_dir)
            .map_err(io_error_with_help("create destination directory", dest_dir))?;
    } else {
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

    // Serialize per-destination directory to avoid races on final rename.
    let _dir_lock = acquire_dir_lock(dest_dir)
        .with_context(|| format!("acquire lock for '{}'", dest_dir.display()))?;

    // Pre-check disk space on destination filesystem for copy fallback.
    check_disk_space(src, dest_dir)?;

    // 1) Try atomic rename.
    match try_atomic_move(src, &dest) {
        Ok(()) => {
            info!(src = %src.display(), dest = %dest.display(), "Renamed file atomically");
            if config.preserve_metadata {
                let meta = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;
                metadata::preserve_metadata(&dest, &meta).ok();
            }
            return Ok(dest);
        }
        Err(e) => {
            #[cfg(unix)]
            let hint: &str = match e
                .downcast_ref::<io::Error>()
                .and_then(|ioe| ioe.raw_os_error())
            {
                Some(code) if code == libc::EXDEV => "cross-filesystem; will copy instead",
                Some(code) if code == libc::EACCES || code == libc::EPERM => {
                    "permission denied; check destination perms"
                }
                _ => "falling back to copy",
            };

            #[cfg(not(unix))]
            let hint: &str = match e.downcast_ref::<io::Error>().map(|ioe| ioe.kind()) {
                Some(io::ErrorKind::PermissionDenied) => {
                    "permission denied; check destination perms"
                }
                _ => "falling back to copy",
            };

            warn!(error = %e, hint, "Atomic rename failed, using safe copy+rename");
        }
    }

    // 2) Fallback: safe copy + fsync + atomic rename, optional metadata.
    safe_copy_and_rename_with_metadata(src, &dest, config.preserve_metadata)?;
    fs::remove_file(src).map_err(io_error_with_help("remove original file", src))?;
    Ok(dest)
}
