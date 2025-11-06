//! Disk space checks.
//! - On Unix, estimates source size and compares it to available space using statvfs.
//! - No-op on non-Unix platforms (returns Ok(())). 
//!
//! Notes:
//! - This function only checks; it does not create directories.
//! - For directories, the total byte size is the sum of regular files in the tree.

use anyhow::{bail, Result};
use std::path::Path;

#[cfg(unix)]
use walkdir::WalkDir;

#[cfg(unix)]
use std::fs;

#[cfg(unix)]
pub(super) fn check_disk_space(src: &Path, dest_dir: &Path) -> Result<()> {
    // Estimate source size (file or directory tree).
    let src_size: u128 = if src.is_file() {
        fs::metadata(src)?.len() as u128
    } else {
        WalkDir::new(src)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len() as u128)
            .sum::<u128>()
    };

    // Query free space on destination filesystem.
    use libc::statvfs;
    use std::ffi::CString;
    use std::io;

    // Build a C string from the raw bytes of the path to avoid lossy conversion.
    use std::os::unix::ffi::OsStrExt;
    let dest_bytes = dest_dir.as_os_str().as_bytes();
    let dest_c = CString::new(dest_bytes)
        .map_err(|e| anyhow::anyhow!("Invalid destination path '{}': {}", dest_dir.display(), e))?;

    let mut stat: statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(dest_c.as_ptr(), &mut stat) };
    if rc != 0 {
        let os_err = io::Error::last_os_error();
        bail!(
            "Failed to stat filesystem for {}: {}",
            dest_dir.display(),
            os_err
        );
    }

    let available: u128 = (stat.f_bavail as u128).saturating_mul(stat.f_frsize as u128);
    if src_size > available {
        bail!(
            "Insufficient space on destination: need {} bytes, have {} bytes",
            src_size,
            available
        );
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn check_disk_space(_src: &Path, _dest_dir: &Path) -> Result<()> {
    // Not implemented on non-Unix platforms.
    Ok(())
}
