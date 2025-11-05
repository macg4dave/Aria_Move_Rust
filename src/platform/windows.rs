//! Windows implementations of platform helpers (best-effort, minimal ACL awareness).

use anyhow::{bail, Result};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

/// Open log file for appending (no symlink defense on Windows, best-effort).
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    OpenOptions::new().create(true).append(true).open(path)
}

/// Write a new config file (overwrite if missing) with best-effort security.
/// Windows does not support POSIX-like mode semantics; we simply write the file.
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    if path.exists() {
        bail!("Config file already exists: {}", path.display());
    }
    fs::write(path, contents)?;
    Ok(())
}

/// No-op on Windows; permission bits are not POSIX-like.
pub fn set_dir_mode_0700(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// No-op on Windows; permission bits are not POSIX-like.
pub fn set_file_mode_0600(_path: &Path) -> io::Result<()> {
    Ok(())
}

/// Minimal security check: ensure path is not readonly. ACL checks are out of scope.
pub fn ensure_secure_directory(path: &Path, label: &str) -> Result<()> {
    let meta = fs::metadata(path)?;
    if meta.permissions().readonly() {
        bail!(
            "{} '{}' has READONLY permissions; cannot write",
            label,
            path.display()
        );
    }
    // Note: For full ACL checks, integrate `icacls` or Windows APIs.
    Ok(())
}

/// No disk-space estimation on Windows in this helper (NYI).
pub fn check_disk_space(_src: &Path, _dest_dir: &Path) -> Result<()> {
    Ok(())
}
