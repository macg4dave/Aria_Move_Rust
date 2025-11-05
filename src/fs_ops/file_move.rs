//! File move implementation.
//! Attempts atomic rename; on cross-filesystem or errors, falls back to safe copy+rename.
//! Preserves metadata if requested and uses an advisory move lock.

use anyhow::{bail, Result};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{info, warn};

use crate::config::Config;
use crate::shutdown;
use crate::utils::{ensure_not_base, stable_file_probe, unique_destination};

use super::atomic::try_atomic_move;
use super::copy::safe_copy_and_rename_with_metadata;
use super::disk::check_disk_space;
use super::helpers::io_error_with_help;
use super::lock::acquire_move_lock;
use super::meta::maybe_preserve_metadata;

/// Move a single file into `completed_base`.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    if shutdown::is_requested() {
        bail!("shutdown requested");
    }

    let _move_lock = acquire_move_lock(src)?;
    ensure_not_base(&config.download_base, src)?;
    stable_file_probe(src, Duration::from_millis(200), 3)?;

    let dest_dir = &config.completed_base;
    if !config.dry_run {
        std::fs::create_dir_all(dest_dir)
            .map_err(io_error_with_help("create destination directory", dest_dir))?;
    } else {
        info!(action = "mkdir -p", path = %dest_dir.display(), "dry-run");
        if let Some(parent) = dest_dir.parent() {
            if !(parent.exists() && !parent.metadata()?.permissions().readonly()) {
                bail!(
                    "dry-run check: cannot create {} (parent missing or readonly)",
                    dest_dir.display()
                );
            }
        }
        // Choose an example destination to report to caller
        let file_name = src
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Source file missing a file name: {}", src.display()))?;
        let mut dest = dest_dir.join(file_name);
        if dest.exists() {
            dest = unique_destination(&dest);
        }
        return Ok(dest);
    }

    let file_name = src
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("Source file missing a file name: {}", src.display()))?;
    let mut dest = dest_dir.join(file_name);
    if dest.exists() {
        dest = unique_destination(&dest);
    }

    if config.dry_run {
        info!(src = %src.display(), dest = %dest.display(), "dry-run: would move file");
        return Ok(dest);
    }

    check_disk_space(src, dest_dir)?;

    match try_atomic_move(src, &dest) {
        Ok(()) => {
            info!(src = %src.display(), dest = %dest.display(), "Renamed file atomically");
            maybe_preserve_metadata(src, &dest, config.preserve_metadata)?;
            Ok(dest)
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
            safe_copy_and_rename_with_metadata(src, &dest, config.preserve_metadata)?;
            std::fs::remove_file(src).map_err(io_error_with_help("remove original file", src))?;
            Ok(dest)
        }
    }
}
