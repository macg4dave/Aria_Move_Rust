//! Unix (macOS/Linux) implementations of platform helpers.

use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Open log file for appending with 0600 permissions.
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
}

/// Write config atomically: temp file (0600) + fsync + rename + fsync dir.
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create parent '{}'", parent.display()))?;

    let tmp = parent.join(format!(
        ".aria_move.config.tmp.{}.{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));

    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("create temp '{}'", tmp.display()))?;
    f.write_all(contents).context("write temp config")?;
    f.sync_all().context("fsync temp config")?;
    drop(f);

    fs::rename(&tmp, path).with_context(|| {
        format!("rename '{}' -> '{}'", tmp.display(), path.display())
    })?;

    let dir_file = File::open(parent).with_context(|| format!("open dir '{}'", parent.display()))?;
    dir_file.sync_all().context("fsync parent dir")?;
    Ok(())
}

/// POSIX chmod 0700 for directories.
pub fn set_dir_mode_0700(path: &Path) -> io::Result<()> {
    let perm = fs::Permissions::from_mode(0o700);
    fs::set_permissions(path, perm)
}

/// POSIX chmod 0600 for files.
pub fn set_file_mode_0600(path: &Path) -> io::Result<()> {
    let perm = fs::Permissions::from_mode(0o600);
    fs::set_permissions(path, perm)
}

/// Create a hidden sibling temp name for atomic writes.
fn tmp_sibling_name(target: &Path) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(".aria_move.config.tmp.{pid}.{nanos}");
    target
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}

/// Check available disk space at the given path (returns bytes available).
/// Uses statvfs on Unix. Returns Ok(available_bytes) or an IO error.
pub fn check_disk_space(path: &Path) -> io::Result<u64> {
    #[cfg(target_os = "macos")]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(path.as_os_str().to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "path not valid UTF-8")
        })?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))?;

        unsafe {
            let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
            if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            let stat = stat.assume_init();
            // Cast both f_bavail and f_bsize to u64 before multiplying
            Ok((stat.f_bavail as u64).saturating_mul(stat.f_bsize as u64))
        }
    }

    #[cfg(target_os = "linux")]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;

        let c_path = CString::new(path.as_os_str().to_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "path not valid UTF-8")
        })?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))?;

        unsafe {
            let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
            if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            let stat = stat.assume_init();
            // f_bavail and f_bsize are both c_ulong; cast to u64 for consistent return type.
            Ok((stat.f_bavail as u64).saturating_mul(stat.f_bsize as u64))
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Fallback for other Unix: return a large number (no check)
        Ok(u64::MAX)
    }
}
