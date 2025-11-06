//! File move implementation.
//! Attempts atomic rename; on cross-filesystem or errors, falls back to safe copy+rename.
//! Preserves metadata if requested and uses an advisory move lock.

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
use super::lock::{acquire_move_lock, acquire_dir_lock, io_error_with_help};
use super::metadata;
use super::{claim, io_copy, space, util};
use crate::platform::check_disk_space;

/// Move a single file into `completed_base`.
pub fn move_file(config: &Config, src: &Path) -> Result<PathBuf> {
    if shutdown::is_requested() {
        return Err(AriaMoveError::Interrupted.into());
    }

    // Acquire per-file move lock
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
                return Err(AriaMoveError::PermissionDenied {
                    path: dest_dir.to_path_buf(),
                    context: "dry-run parent missing or readonly".into(),
                }
                .into());
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
            if config.preserve_metadata {
                let meta = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;
                metadata::preserve_metadata(&dest, &meta).ok();
            }
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

/// Move a single file into `completed_base`.
pub fn move_file_old(cfg: &Config, src: &Path) -> Result<()> {
    if !src.exists() {
        return Err(anyhow!("source does not exist: {}", src.display()));
    }

    let name = src
        .file_name()
        .ok_or_else(|| anyhow!("source path has no file name: {}", src.display()))?;
    let dst_dir = &cfg.completed_base;
    let dst_path = dst_dir.join(name);

    if cfg.dry_run {
        println!("Dry-run: would move '{}' -> '{}'", src.display(), dst_path.display());
        return Ok(());
    }

    // Ensure destination directory exists
    if !dst_dir.exists() {
        fs::create_dir_all(dst_dir)
            .with_context(|| format!("create dest dir '{}'", dst_dir.display()))?;
    }

    // Serialize per-destination directory
    let _lock = acquire_dir_lock(dst_dir)
        .with_context(|| format!("acquire lock for '{}'", dst_dir.display()))?;

    // Claim the source atomically (single winner)
    let claimed_src = match claim::claim_source(src) {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if dst_path.exists() {
                return Ok(());
            }
            return Err(anyhow!(
                "source disappeared before claim ({}); destination also missing",
                src.display()
            ));
        }
        Err(e) => return Err(anyhow!("failed to claim source '{}': {e}", src.display())),
    };

    // Stable metadata
    let meta_before = fs::metadata(&claimed_src)
        .with_context(|| format!("stat {}", claimed_src.display()))?;

    // Try same-fs atomic rename
    match fs::rename(&claimed_src, &dst_path) {
        Ok(()) => {
            if cfg.preserve_metadata {
                metadata::preserve_metadata(&dst_path, &meta_before).ok();
            }
            util::fsync_dir(dst_dir).ok();
            return Ok(());
        }
        Err(e) => {
            if !util::is_cross_device(&e) {
                // best-effort rollback
                let _ = fs::rename(&claimed_src, src);
                return Err(anyhow!(
                    "rename '{}' -> '{}' failed: {e}",
                    claimed_src.display(),
                    dst_path.display()
                ));
            }
            // Cross-device -> fallback
        }
    }

    // Disk-space check before copy
    let required = meta_before.len();
    space::ensure_space_for_copy(dst_dir, required).with_context(|| {
        format!(
            "insufficient free space on destination filesystem (need ~{}, free {})",
            space::format_bytes(required),
            space::format_bytes(space::free_space_bytes(dst_dir).unwrap_or(0))
        )
    })?;

    // Copy -> fsync -> (optional) metadata -> atomic rename -> fsync dir
    let tmp = util::unique_temp_path(dst_dir);
    io_copy::copy_streaming(&claimed_src, &tmp).with_context(|| {
        format!(
            "copy (fallback) '{}' -> '{}'",
            claimed_src.display(),
            tmp.display()
        )
    })?;

    let mut tmp_f = File::open(&tmp)?;
    tmp_f.sync_all()?;

    if cfg.preserve_metadata {
        metadata::preserve_metadata(&tmp, &meta_before).ok();
    }

    fs::rename(&tmp, &dst_path).with_context(|| {
        format!(
            "rename (finalize) '{}' -> '{}'",
            tmp.display(),
            dst_path.display()
        )
    })?;

    util::fsync_dir(dst_dir).ok();

    // Remove claimed src (we copied it)
    if let Err(e) = fs::remove_file(&claimed_src) {
        eprintln!(
            "Warning: destination in place but failed to remove source '{}': {}",
            claimed_src.display(),
            e
        );
    }

    Ok(())
}
