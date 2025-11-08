//! Miscellaneous filesystem helpers used across fs_ops.
//!
//! - unique_temp_path: generate a unique temporary path inside a destination directory
//! - is_cross_device: detect cross-filesystem rename errors (EXDEV/ERROR_NOT_SAME_DEVICE)
//! - fsync_dir: best-effort directory fsync after a rename (Unix only)

// remove unused File import
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a unique temp path within `dst_dir`.
/// The file is not created here; callers typically pass the returned path to a function
/// that uses create_new(true) to avoid clobbering an existing file.
/// 
/// Format: ".aria_move.<pid>.<nanos>[.<attempt>].tmp"
pub(super) fn unique_temp_path(dst_dir: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Try a few attempts in the astronomically unlikely case of a collision.
    // We do NOT touch the filesystem here; io_copy will still open with create_new(true).
    const MAX_TRIES: u32 = 5;
    for attempt in 0..=MAX_TRIES {
        let name = if attempt == 0 {
            format!(".aria_move.{pid}.{nanos}.tmp")
        } else {
            format!(".aria_move.{pid}.{nanos}.{attempt}.tmp")
        };
        let candidate = dst_dir.join(name);
        // Best-effort quick check to avoid a guaranteed AlreadyExists; not a race barrier.
        if !candidate.exists() {
            return candidate;
        }
    }

    // Final fallback (should virtually never happen).
    dst_dir.join(format!(
        ".aria_move.{pid}.{nanos}.final.tmp"
    ))
}

/// Return true if an io::Error represents a cross-device rename (EXDEV on Unix, NOT_SAME_DEVICE on Windows).
pub(super) fn is_cross_device(e: &io::Error) -> bool {
    if let Some(code) = e.raw_os_error() {
        #[cfg(unix)]
        {
            // libc::EXDEV is the canonical code for cross-device link.
            if code == libc::EXDEV {
                return true;
            }
        }
        #[cfg(windows)]
        {
            // ERROR_NOT_SAME_DEVICE = 17
            const ERROR_NOT_SAME_DEVICE: i32 = 17;
            if code == ERROR_NOT_SAME_DEVICE {
                return true;
            }
        }
    }
    false
}

/// Best-effort fsync of a directory (persists a completed rename) — Unix only.
/// On Windows, this is a no-op (directory handles can’t be fsynced portably).
#[cfg(unix)]
pub(super) fn fsync_dir(dir: &Path) -> io::Result<()> {
    use std::fs::File;
    let f = File::open(dir)?;
    f.sync_all()
}

#[cfg(windows)]
pub(super) fn fsync_dir(_dir: &Path) -> io::Result<()> {
    Ok(())
}