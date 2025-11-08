//! Miscellaneous filesystem helpers used across fs_ops.
//!
//! - unique_temp_path: generate a unique temporary path inside a destination directory
//! - is_cross_device: detect cross-filesystem rename errors (EXDEV/ERROR_NOT_SAME_DEVICE)
//! - fsync_dir: best-effort directory fsync after a rename (Unix only)

// remove unused File import
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
// no longer need timestamp imports; deterministic resume temp uses hashing

// unique_temp_path removed in favor of deterministic resume_temp_path.

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

/// Deterministic resume temp path for a given final destination.
/// Format: ".aria_move.resume.<hexhash>.tmp" where hash is of the absolute dest path.
/// Public for use in integration tests to simulate partial copies.
pub fn resume_temp_path(dest: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    // Hash the full, lossy-display path for stability across runs.
    // Canonicalization is optional; use as-provided to match caller's computed dest.
    dest.to_string_lossy().hash(&mut hasher);
    let h = hasher.finish();
    let name = format!(".aria_move.resume.{:016x}.tmp", h);
    match dest.parent() {
        Some(p) => p.join(name),
        None => PathBuf::from(name),
    }
}
