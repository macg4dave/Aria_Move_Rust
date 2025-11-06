//! Windows implementations of platform helpers (best-effort, minimal ACL awareness).
//!
//! Notes:
//! - Windows lacks POSIX mode semantics; we do not attempt ACL management here.
//! - We avoid following symlinks only where std allows (limited on Windows).
//! - Config writes are done via temp + rename to be atomic.

use anyhow::{bail, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Open log file for appending (best-effort; no symlink defense available via std on Windows).
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Write a new config file atomically (create_new) using a temp file + rename.
/// Fails if the target already exists. Best-effort security (no ACL changes).
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    if path.exists() {
        bail!("Config file already exists: {}", path.display());
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;

    // Create a unique sibling temp file, write, fsync, then rename into place.
    let tmp = tmp_sibling_name(path);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    f.write_all(contents)?;
    f.sync_all()?; // ensure data is on disk before renaming
    fs::rename(&tmp, path)?;
    // Note: On Windows, fsync of the parent directory is not generally supported via std.
    Ok(())
}

/// No-op on Windows; POSIX-style directory modes are not applicable.
pub fn set_dir_mode_0700(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// No-op on Windows; POSIX-style file modes are not applicable.
pub fn set_file_mode_0600(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Minimal security check: path must be an existing, non-readonly directory.
/// Full ACL verification is out of scope.
pub fn ensure_secure_directory(path: &Path, label: &str) -> Result<()> {
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        bail!("{} '{}' is not a directory", label, path.display());
    }
    if meta.permissions().readonly() {
        bail!(
            "{} '{}' has READONLY permissions; cannot write",
            label,
            path.display()
        );
    }
    Ok(())
}

/// No disk-space estimation on Windows in this helper (NYI).
pub fn check_disk_space(_src: &Path, _dest_dir: &Path) -> Result<()> {
    Ok(())
}

/// Create a sibling temporary filename for atomic write/rename.
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
