//! Free-space helpers and human-readable byte formatting.
//!
//! Responsibilities:
//! - `free_space_bytes`: platform-specific free space query on the filesystem of a path.
//! - `ensure_space_for_copy`: guard enforcing a small cushion beyond required bytes (to cover metadata, journal, temp files).
//! - `format_bytes`: compact, human-friendly formatting for diagnostics.
//! - `has_space`: pure helper for deterministic unit testing of space logic.
//!
//! Design notes:
//! - A fixed cushion (`SPACE_CUSHION_BYTES`) avoids borderline failures when post-copy metadata updates or temp files consume additional blocks.
//! - `ensure_space_for_copy` treats a non-existent destination path as a prospective file and falls back to its parent directory for the space check.
//! - Space checks are inherently racy; the functions provide a best-effort pre-flight validation only.
//! - We use `f_bavail` (user-available blocks) rather than `f_bfree` on Unix for conservative estimation.
//! - Formatting trims trailing `.0` for cleaner output (e.g. `1 GiB` instead of `1.0 GiB`).
//!
//! Potential future enhancements:
//! - Make cushion configurable from a higher-level config.
//! - Add an error variant instead of generic anyhow.
//! - Expose raw bytes in error metadata (already embedded via formatting).

use crate::errors::AriaMoveError;
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Binary-unit formatting (KiB/MiB/GiB) rounded to one decimal; trims trailing `.0`.
pub(super) fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let f = n as f64;
    let (value, unit) = if f >= GB {
        (f / GB, "GiB")
    } else if f >= MB {
        (f / MB, "MiB")
    } else if f >= KB {
        (f / KB, "KiB")
    } else {
        return format!("{} B", n);
    };
    let formatted = format!("{:.1}", value);
    let trimmed = if formatted.ends_with(".0") {
        &formatted[..formatted.len() - 2]
    } else {
        &formatted
    };
    format!("{} {}", trimmed, unit)
}

/// Fixed cushion in bytes added to required amount when validating free space.
pub(super) const SPACE_CUSHION_BYTES: u64 = 4 * 1024 * 1024; // 4 MiB

/// Pure function: returns true if `free` bytes is >= required + cushion.
pub(super) fn has_space(free: u64, required: u64) -> bool {
    free >= required.saturating_add(SPACE_CUSHION_BYTES)
}

/// Ensure the destination filesystem has at least `required` bytes plus a small cushion.
/// The cushion helps avoid borderline failures from metadata, journal, and temp usage.
pub(super) fn ensure_space_for_copy(dst_dir: &Path, required: u64) -> Result<(), AriaMoveError> {
    // Resolve actual directory for the free space query:
    // - If `dst_dir` exists and is a directory: use it directly.
    // - If it does not exist: attempt its parent (prospective file case).
    // - Otherwise: fall back to given path (error will be propagated by stat call if invalid).
    let query_path: &Path = if dst_dir.is_dir() {
        dst_dir
    } else if !dst_dir.exists() {
        dst_dir.parent().unwrap_or(dst_dir)
    } else {
        dst_dir // Exists but not directory (caller may have passed file path); use its parent if present.
    };

    let free = free_space_bytes(query_path).map_err(|_| AriaMoveError::InsufficientSpace {
        required: (required as u128).saturating_add(SPACE_CUSHION_BYTES as u128),
        available: 0u128,
        dest: query_path.to_path_buf(),
    })?;
    if !has_space(free, required) {
        return Err(AriaMoveError::InsufficientSpace {
            required: (required as u128).saturating_add(SPACE_CUSHION_BYTES as u128),
            available: free as u128,
            dest: query_path.to_path_buf(),
        });
    }
    Ok(())
}

/// Return available free space (in bytes) on the filesystem hosting `path`.
#[cfg(unix)]
pub(super) fn free_space_bytes(path: &Path) -> io::Result<u64> {
    use libc::statvfs;

    let mut s: statvfs = unsafe { std::mem::zeroed() };
    let cpath = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;

    let rc = unsafe { libc::statvfs(cpath.as_ptr(), &mut s) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    // On some platforms (e.g., older macOS), f_frsize may be 0; fall back to f_bsize.
    // Use `u64::from` to avoid redundant casts where the underlying type is already u64.
    let block_size: u64 = if s.f_frsize != 0 {
        s.f_frsize
    } else {
        s.f_bsize
    };
    Ok(s.f_bavail.saturating_mul(block_size))
}

/// Return available free space (in bytes) on the filesystem hosting `path`.
#[cfg(windows)]
pub(super) fn free_space_bytes(path: &Path) -> io::Result<u64> {
    use std::iter::once;
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(once(0)).collect();

    let mut free_avail: u64 = 0;
    let mut _total: u64 = 0;
    let mut _total_free: u64 = 0;

    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_avail as *mut u64,
            &mut _total as *mut u64,
            &mut _total_free as *mut u64,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(free_avail)
}

/// Fallback for unsupported targets: report “unsupported”.
#[cfg(not(any(unix, windows)))]
pub(super) fn free_space_bytes(_path: &Path) -> io::Result<u64> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "free space query not supported on this platform",
    ))
}

// ---------- Tests ----------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_boundaries() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1 KiB");
        assert_eq!(format_bytes(1024 * 1024), "1 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5 MiB");
    }

    #[test]
    fn has_space_logic() {
        let cushion = SPACE_CUSHION_BYTES;
        assert!(has_space(cushion, 0)); // Free == cushion, required 0
        assert!(has_space(cushion + 1, 1));
        assert!(!has_space(cushion - 1, 0));
        // Near saturation
        let max = u64::MAX;
        assert!(has_space(max, max - cushion));
    }

    #[test]
    fn ensure_space_for_copy_parent_fallback() {
        // Use a temp directory and pass a prospective file path (non-existent).
        let dir = tempfile::tempdir().unwrap();
        let prospective = dir.path().join("future_file.bin");
        // Required 1 byte should pass (unless disk is almost full).
        ensure_space_for_copy(&prospective, 1).unwrap();
    }

    // Helper to exercise the error path deterministically without relying on actual disk space.
    #[track_caller]
    fn simulate_insufficient(
        query_path: &Path,
        free: u64,
        required: u64,
    ) -> Result<(), AriaMoveError> {
        if !has_space(free, required) {
            return Err(AriaMoveError::InsufficientSpace {
                required: (required as u128).saturating_add(SPACE_CUSHION_BYTES as u128),
                available: free as u128,
                dest: query_path.to_path_buf(),
            });
        }
        Ok(())
    }

    #[test]
    fn insufficient_space_error_variant() {
        let dir = tempfile::tempdir().unwrap();
        let prospective = dir.path().join("file.bin");
        // Choose free smaller than cushion to force error for any required > 0
        let free = SPACE_CUSHION_BYTES - 1;
        let required = 1u64;
        let err = simulate_insufficient(&prospective, free, required).unwrap_err();
        match err {
            AriaMoveError::InsufficientSpace {
                required: need,
                available,
                dest,
            } => {
                assert_eq!(need, (required as u128) + (SPACE_CUSHION_BYTES as u128));
                assert_eq!(available, free as u128);
                assert_eq!(dest, prospective);
            }
            _ => panic!("unexpected error variant"),
        }
    }
}
