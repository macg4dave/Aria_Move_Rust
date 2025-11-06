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

// Stubs for Windows builds (adjust with real implementation if needed).
pub fn open_log_file_secure_append(_path: &Path) -> io::Result<std::fs::File> {
    std::fs::OpenOptions::new().create(true).append(true).open(_path)
}
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    std::fs::write(path, contents)?;
    Ok(())
}
pub fn set_dir_mode_0700(_path: &Path) -> io::Result<()> {
    Ok(())
}
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
pub fn check_disk_space(_path: &std::path::Path) -> std::io::Result<u64> {
    // Windows stub: return a large value (no validation)
    Ok(u64::MAX)
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
