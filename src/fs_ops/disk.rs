//! Disk space checks (Unix).
//! Estimates source size and compares it to available space using statvfs; no-op on non-Unix.

use anyhow::{bail, Result};
use std::path::Path;

#[cfg(unix)]
use walkdir::WalkDir;

#[cfg(unix)]
use std::fs;

#[cfg(unix)]
pub(super) fn check_disk_space(src: &Path, dest_dir: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    // Normalize to u128 to avoid cross-platform integer mismatches.
    let src_size: u128 = if src.is_file() {
        fs::metadata(src)?.size() as u128
    } else {
        WalkDir::new(src)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.size() as u128)
            .sum::<u128>()
    };

    use libc::statvfs;
    use std::ffi::CString;
    let dest_c = CString::new(dest_dir.to_string_lossy().into_owned())
        .map_err(|e| anyhow::anyhow!("Invalid destination path '{}': {}", dest_dir.display(), e))?;
    let mut stat: statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(dest_c.as_ptr(), &mut stat) };
    if rc != 0 {
        bail!("Failed to stat filesystem for {}", dest_dir.display());
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
