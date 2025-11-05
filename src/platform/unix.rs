//! Unix implementations of platform helpers.

use anyhow::{bail, Result};
use libc;
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;

use walkdir::WalkDir;

/// Open a log file safely for appending, refusing to follow symlinks, 0600 perms.
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    let mut opts = OpenOptions::new();
    opts.create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW);
    opts.open(path)
}

/// Write a new config file atomically with 0600 perms and no symlink following.
/// Fails if the file already exists.
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    let mut opts = OpenOptions::new();
    let mut f = opts
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?;
    f.write_all(contents)?;
    Ok(())
}

/// chmod 0700 on a directory (best effort).
pub fn set_dir_mode_0700(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

/// chmod 0600 on a file (best effort).
pub fn set_file_mode_0600(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}

/// Ensure directory is owned by the current euid and not group/world writable.
pub fn ensure_secure_directory(path: &Path, label: &str) -> Result<()> {
    let meta = fs::metadata(path)?;
    if meta.permissions().mode() & 0o022 != 0 {
        bail!(
            "{} '{}' is group/world-writable; insecure directory",
            label,
            path.display()
        );
    }
    let euid = unsafe { libc::geteuid() };
    if meta.uid() != euid {
        bail!(
            "{} '{}' is not owned by current user (uid {})",
            label,
            path.display(),
            euid
        );
    }
    Ok(())
}

/// Check that the destination filesystem has at least the size of `src` available.
/// On directories, sums file sizes. Compares using statvfs.
pub fn check_disk_space(src: &Path, dest_dir: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

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

    let dest_c = CString::new(dest_dir.to_string_lossy().into_owned())
        .map_err(|e| anyhow::anyhow!("Invalid destination path '{}': {}", dest_dir.display(), e))?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
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
