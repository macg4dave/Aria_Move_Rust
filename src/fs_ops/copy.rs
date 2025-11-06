//! Safe copy-and-rename helper:
//! - Copies to a temp file in the destination directory
//! - Ensures data durability (io_copy::copy_streaming fsyncs the temp file)
//! - Atomically renames temp -> dest (Windows overwrite-safe)
//! - Fsyncs the destination directory (Unix; handled in atomic::try_atomic_move)

use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::Path;

use super::{io_copy, metadata, util};
use super::atomic::try_atomic_move;
use super::lock::io_error_with_help;

/// Core: copy src -> temp in dest dir, then atomic rename temp -> dest.
/// Notes:
/// - io_copy::copy_streaming creates the temp file with O_EXCL and fsyncs it before returning.
/// - try_atomic_move handles Windows "overwrite" and fsyncs the destination directory on Unix.
pub fn safe_copy_and_rename(src: &Path, dest: &Path) -> Result<()> {
    let dest_dir = dest
        .parent()
        .ok_or_else(|| anyhow!("destination has no parent: {}", dest.display()))?;

    // Ensure destination directory exists.
    fs::create_dir_all(dest_dir)
        .map_err(io_error_with_help("create destination directory", dest_dir))?;

    // Allocate a unique temp path within the destination directory.
    let tmp_path = util::unique_temp_path(dest_dir);

    // Stream the copy (fsyncs temp file internally).
    io_copy::copy_streaming(src, &tmp_path)
        .map_err(io_error_with_help("copy to temporary file", &tmp_path))?;

    // Atomic rename into final destination (handles Windows overwrite + Unix dir fsync).
    if let Err(e) = try_atomic_move(&tmp_path, dest) {
        // Best-effort cleanup of the temp file on failure.
        let _ = fs::remove_file(&tmp_path);
        return Err(e).with_context(|| {
            format!(
                "rename temporary file '{}' -> '{}'",
                tmp_path.display(),
                dest.display()
            )
        });
    }

    Ok(())
}

/// Wrapper: perform safe copy-and-rename, then preserve metadata if requested.
pub fn safe_copy_and_rename_with_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    safe_copy_and_rename(src, dest)?;
    if preserve {
        let meta = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;
        metadata::preserve_metadata(dest, &meta)
            .with_context(|| format!("preserve metadata for {}", dest.display()))?;
    }
    Ok(())
}
