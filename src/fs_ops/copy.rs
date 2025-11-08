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
use super::io_error_with_help;

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

    // Choose deterministic resume temp path inside destination directory.
    let tmp_path = util::resume_temp_path(dest);

    // Determine sizes
    let src_size = fs::metadata(src)
        .with_context(|| format!("stat {}", src.display()))?
        .len();
    let tmp_len = fs::metadata(&tmp_path).map(|m| m.len()).ok();

    // If a previous partial exists, resume; else perform fresh copy.
    if let Some(existing) = tmp_len {
        if existing > src_size {
            // Corrupted temp (larger than source) — start fresh
            let _ = fs::remove_file(&tmp_path);
        } else if existing == src_size {
            // Already fully copied; just finalize
            if let Err(e) = try_atomic_move(&tmp_path, dest) {
                // Best-effort cleanup on failure
                let _ = fs::remove_file(&tmp_path);
                return Err(e).with_context(|| {
                    format!(
                        "rename temporary file '{}' -> '{}'",
                        tmp_path.display(),
                        dest.display()
                    )
                });
            }
            return Ok(());
        } else {
            // Resume from existing offset
            let res = io_copy::copy_streaming_resume(src, &tmp_path, existing)
                .map_err(io_error_with_help("resume copy to temporary file", &tmp_path))?;
            if res != src_size {
                // Incomplete resume; treat as error and cleanup
                let _ = fs::remove_file(&tmp_path);
                return Err(anyhow!(
                    "resume short write: wrote {} bytes but source is {} bytes",
                    res,
                    src_size
                ));
            }
            // Finalize rename
            if let Err(e) = try_atomic_move(&tmp_path, dest) {
                let _ = fs::remove_file(&tmp_path);
                return Err(e).with_context(|| {
                    format!(
                        "rename temporary file '{}' -> '{}'",
                        tmp_path.display(),
                        dest.display()
                    )
                });
            }
            return Ok(());
        }
    }

    // Fresh copy path
    let written = io_copy::copy_streaming(src, &tmp_path)
        .map_err(io_error_with_help("copy to temporary file", &tmp_path))?;
    if written != src_size {
        let _ = fs::remove_file(&tmp_path);
        return Err(anyhow!(
            "short write while copying: wrote {} bytes but source is {} bytes",
            written,
            src_size
        ));
    }
    if let Err(e) = try_atomic_move(&tmp_path, dest) {
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
/// When `strict` is true and `preserve` is true, any failure to preserve metadata returns an error.
pub fn safe_copy_and_rename_with_metadata(src: &Path, dest: &Path, preserve: bool) -> Result<()> {
    safe_copy_and_rename(src, dest)?;
    if preserve {
        let meta = fs::metadata(src).with_context(|| format!("stat {}", src.display()))?;
        metadata::preserve_metadata(dest, &meta)
            .with_context(|| format!("preserve metadata for {}", dest.display()))?;
        // Preserve xattrs as part of "preserve everything" when enabled
        metadata::preserve_xattrs(src, dest)
            .with_context(|| format!("preserve xattrs for {}", dest.display()))?;
    }
    Ok(())
}
