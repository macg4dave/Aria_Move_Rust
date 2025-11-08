//! Atomic rename helper.
//! - Performs a rename with context-rich errors.
//! - On Windows, removes an existing destination first (RenameFile doesn’t overwrite).
//! - On Unix, best-effort fsync of the destination directory after rename.

use anyhow::{Context, Result};
use tracing::debug;

/// Outcome of an attempted atomic move.
/// - Renamed: atomic rename completed on the same filesystem.
/// - CrossDevice: pre-detected cross-filesystem move; caller should copy instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOutcome {
    Renamed,
    CrossDevice,
}
use std::fs;
use std::path::Path;

pub fn try_atomic_move(src: &Path, dst: &Path) -> Result<MoveOutcome> {
    // Unix: pre-detect cross-device moves to avoid a failing rename with EXDEV.
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if let (Some(src_parent), Some(dst_parent)) = (src.parent(), dst.parent())
            && let (Ok(s_meta), Ok(d_meta)) = (fs::metadata(src_parent), fs::metadata(dst_parent))
            && s_meta.dev() != d_meta.dev()
        {
            return Ok(MoveOutcome::CrossDevice);
        }
    }

    // Windows: ensure destination path is free (rename doesn’t overwrite there).
    #[cfg(windows)]
    {
        if dst.exists() {
            // Best-effort removal; propagate unexpected errors with context.
            if let Err(e) = fs::remove_file(dst) {
                // If not found, ignore; otherwise return enriched error.
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e).with_context(|| {
                        format!(
                            "remove existing destination before rename: {}",
                            dst.display()
                        )
                    });
                }
            }
        }
    }

    // Perform the atomic rename.
    fs::rename(src, dst)
        .with_context(|| format!("atomic rename '{}' -> '{}'", src.display(), dst.display()))?;

    // Unix: fsync directories to persist the rename (best-effort).
    #[cfg(unix)]
    {
        // Ignore fsync errors to avoid turning a successful rename into a failure.
        if let Some(dst_parent) = dst.parent()
            && let Err(e) = super::util::fsync_dir(dst_parent)
        {
            debug!(error = %e, dir = %dst_parent.display(), "best-effort fsync(dst_parent) failed");
        }
        if let (Some(src_parent), Some(dst_parent)) = (src.parent(), dst.parent())
            && src_parent != dst_parent
            && let Err(e) = super::util::fsync_dir(src_parent)
        {
            debug!(error = %e, dir = %src_parent.display(), "best-effort fsync(src_parent) failed");
        }
    }

    Ok(MoveOutcome::Renamed)
}
