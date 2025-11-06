//! Atomic rename helper.
//! - Performs a rename with context-rich errors.
//! - On Windows, removes an existing destination first (RenameFile doesn’t overwrite).
//! - On Unix, best-effort fsync of the destination directory after rename.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

pub fn try_atomic_move(src: &Path, dst: &Path) -> Result<()> {
    // Windows: ensure destination path is free (rename doesn’t overwrite there).
    #[cfg(windows)]
    {
        if dst.exists() {
            // Best-effort removal; propagate unexpected errors with context.
            if let Err(e) = fs::remove_file(dst) {
                // If not found, ignore; otherwise return enriched error.
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(e).with_context(|| {
                        format!("remove existing destination before rename: {}", dst.display())
                    });
                }
            }
        }
    }

    // Perform the atomic rename.
    fs::rename(src, dst)
        .with_context(|| format!("atomic rename '{}' -> '{}'", src.display(), dst.display()))?;

    // Unix: fsync the destination directory to persist the rename (best-effort).
    #[cfg(unix)]
    if let Some(parent) = dst.parent() {
        // Ignore fsync errors to avoid turning a successful rename into a failure.
        let _ = super::util::fsync_dir(parent);
    }

    Ok(())
}
