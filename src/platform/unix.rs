//! Unix implementations of platform helpers.
//!
//! Goals:
//! - Do not follow symlinks for sensitive paths (use O_NOFOLLOW).
//! - Use restrictive permissions (0600 files, 0700 dirs).
//! - Provide clear error contexts.
//! - Keep functions side-effect free unless their name implies creation/mutation.

use anyhow::{Context, Result};
use libc;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Open a log file safely for appending without following symlinks, with 0600 perms.
/// Creates the file if it does not exist.
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
}

/// Write a new config file atomically with 0600 perms and no symlink following.
/// Fails if the file already exists (create_new semantics).
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;

    // 1) Create a new temp file next to the target (fail if it exists), no symlink follow.
    let tmp = tmp_sibling_name(path);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(&tmp)
        .with_context(|| format!("create temp config '{}'", tmp.display()))?;

    // 2) Write and fsync the file contents.
    f.write_all(contents)
        .with_context(|| format!("write temp config '{}'", tmp.display()))?;
    f.sync_all()
        .with_context(|| format!("fsync temp config '{}'", tmp.display()))?;

    // 3) Atomically rename into place, then fsync the parent directory to persist the rename.
    fs::rename(&tmp, path)
        .with_context(|| format!("rename '{}' -> '{}'", tmp.display(), path.display()))?;

    // Best-effort directory fsync; do not turn success into failure if it errors.
    let _ = File::open(parent).and_then(|dir| dir.sync_all());

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
    let meta = fs::metadata(path).with_context(|| format!("stat {} '{}'", label, path.display()))?;

    // Reject group/world-writable dirs.
    if meta.permissions().mode() & 0o022 != 0 {
        anyhow::bail!(
            "{} '{}' is group/world-writable; insecure directory",
            label,
            path.display()
        );
    }

    // Require ownership by current user.
    let euid = unsafe { libc::geteuid() };
    if meta.uid() != euid {
        anyhow::bail!(
            "{} '{}' is not owned by current user (uid {})",
            label,
            path.display(),
            euid
        );
    }
    Ok(())
}

/// Check that the destination filesystem has at least the size of `src` available.
/// - For files: uses metadata len.
/// - For directories: sums file sizes via WalkDir.
/// - Compares using statvfs without lossy path conversions.
pub fn check_disk_space(src: &Path, dest_dir: &Path) -> Result<()> {
    let src_size: u128 = if src.is_file() {
        fs::metadata(src)
            .with_context(|| format!("stat source '{}'", src.display()))?
            .len() as u128
    } else {
        WalkDir::new(src)
            .into_iter()
            .filter_map(Result::ok)
            .filter_map(|e| e.metadata().ok())
            .filter(|m| m.is_file())
            .map(|m| m.len() as u128)
            .sum::<u128>()
    };

    // Build a C string from the raw bytes to avoid lossy UTF-8 conversions.
    let c_path = std::ffi::CString::new(dest_dir.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "destination path contains NUL"))
        .with_context(|| format!("prepare path '{}'", dest_dir.display()))?;

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        let os_err = io::Error::last_os_error();
        anyhow::bail!(
            "statvfs failed for '{}': {}",
            dest_dir.display(),
            os_err
        );
    }

    let block = if stat.f_frsize != 0 {
        stat.f_frsize as u128
    } else {
        stat.f_bsize as u128
    };
    let available: u128 = (stat.f_bavail as u128).saturating_mul(block);

    if src_size > available {
        anyhow::bail!(
            "Insufficient space on destination: need {} bytes, have {} bytes",
            src_size,
            available
        );
    }
    Ok(())
}

/// Create a sibling temporary filename for atomic write/rename.
fn tmp_sibling_name(target: &Path) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    let pid = std::process::id();
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let name = format!(
        ".aria_move.config.tmp.{}.{}",
        pid, nanos
    );
    target
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(name)
}
