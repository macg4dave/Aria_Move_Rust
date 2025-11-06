//! Free-space helpers and human-readable byte formatting.
//!
//! - free_space_bytes: platform-specific free space query on the filesystem of a path
//! - ensure_space_for_copy: guard that enforces a small cushion beyond required bytes
//! - format_bytes: compact, human-friendly formatting for diagnostics

use anyhow::anyhow;
use std::io;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

/// Format a byte count using binary units (KiB/MiB/GiB), rounded to one decimal.
pub(super) fn format_bytes(n: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let f = n as f64;
    if f >= GB {
        format!("{:.1} GiB", f / GB)
    } else if f >= MB {
        format!("{:.1} MiB", f / MB)
    } else if f >= KB {
        format!("{:.1} KiB", f / KB)
    } else {
        format!("{} B", n)
    }
}

/// Ensure the destination filesystem has at least `required` bytes plus a small cushion.
/// The cushion helps avoid borderline failures from metadata, journal, and temp usage.
pub(super) fn ensure_space_for_copy(dst_dir: &Path, required: u64) -> anyhow::Result<()> {
    let free = free_space_bytes(dst_dir)?;
    // 4 MiB cushion to avoid off-by-a-bit failures after metadata and temp files.
    const CUSHION: u64 = 4 * 1024 * 1024;

    let need = required.saturating_add(CUSHION);
    if free < need {
        return Err(anyhow!(
            "not enough free space in '{}': need ~{}, free {}",
            dst_dir.display(),
            format_bytes(required),
            format_bytes(free)
        ));
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
    let block_size = if s.f_frsize != 0 { s.f_frsize } else { s.f_bsize } as u64;
    Ok((s.f_bavail as u64).saturating_mul(block_size))
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