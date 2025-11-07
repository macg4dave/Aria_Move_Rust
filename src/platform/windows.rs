//! Windows implementations of platform helpers (best-effort, minimal ACL awareness).
//!
//! Notes:
//! - Windows lacks POSIX mode semantics; we do not attempt ACL management here.
//! - We avoid following symlinks only where std allows (limited on Windows).
//! - Config writes are done via temp + rename to be atomic.
//! - Disk space query uses GetDiskFreeSpaceExW.

use anyhow::{bail, Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, GetFileAttributesW, SetFileAttributesW, DeleteFileW,
    FILE_ATTRIBUTE_READONLY, FILE_ATTRIBUTE_TEMPORARY,
};

/// Open a log file for appending (best-effort; no ACL changes). Ensures the file exists.
pub fn open_log_file_secure_append(path: &Path) -> io::Result<std::fs::File> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    OpenOptions::new().create(true).append(true).open(path)
}

/// Write config atomically using temp + flush + rename. On failure, attempts to clean up temp.
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create parent '{}'", parent.display()))?;

    let tmp = tmp_sibling_name(path);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .with_context(|| format!("create temp '{}'", tmp.display()))?;
    f.write_all(contents).context("write temp config")?;
    f.sync_all().context("flush temp config")?;
    drop(f);

    // Mark the temp file as FILE_ATTRIBUTE_TEMPORARY (best-effort).
    mark_temp_attribute(&tmp);

    // Ensure destination (if it exists) is not read-only; we prefer create_new semantics, but if
    // a previous partial file exists and is READONLY, clear it so we can replace.
    if path.exists() {
        clear_readonly_attribute(path);
        // Remove existing file to make the rename semantics consistent with create_new.
        // Best-effort: ignore errors; rename will surface fatal ones.
        let _ = fs::remove_file(path);
    }

    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("rename '{}' -> '{}'", tmp.display(), path.display()));
    }
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

/// Disk-space estimation using GetDiskFreeSpaceExW.
pub fn check_disk_space(path: &std::path::Path) -> std::io::Result<u64> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(once(0)).collect();
    let mut free_avail: u64 = 0;
    let mut _total: u64 = 0;
    let mut _total_free: u64 = 0;
    let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_avail, &mut _total, &mut _total_free) };
    if ok == 0 { return Err(io::Error::last_os_error()); }
    Ok(free_avail)
}

/// Create a sibling temporary filename for atomic write/rename.
fn tmp_sibling_name(target: &Path) -> PathBuf {
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let pid = std::process::id();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!(".aria_move.config.tmp.{pid}.{nanos}.{seq}");
    target.parent().unwrap_or_else(|| Path::new(".")).join(name)
}

// ---- Attribute helpers (best-effort; ignore errors) ----
fn mark_temp_attribute(p: &Path) {
    if let Some(wide) = to_wide(p) {
        unsafe {
            let current = GetFileAttributesW(wide.as_ptr());
            if current != u32::MAX { // INVALID_FILE_ATTRIBUTES
                let new_attr = current | FILE_ATTRIBUTE_TEMPORARY;
                let _ = SetFileAttributesW(wide.as_ptr(), new_attr);
            }
        }
    }
}

fn clear_readonly_attribute(p: &Path) {
    if let Some(wide) = to_wide(p) {
        unsafe {
            let current = GetFileAttributesW(wide.as_ptr());
            if current != u32::MAX && (current & FILE_ATTRIBUTE_READONLY) != 0 {
                let new_attr = current & !FILE_ATTRIBUTE_READONLY;
                let _ = SetFileAttributesW(wide.as_ptr(), new_attr);
            }
        }
    }
}

#[allow(dead_code)]
fn delete_file_best_effort(p: &Path) {
    if let Some(wide) = to_wide(p) {
        unsafe { let _ = DeleteFileW(wide.as_ptr()); }
    }
}

fn to_wide(p: &Path) -> Option<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let mut v: Vec<u16> = p.as_os_str().encode_wide().collect();
    v.push(0);
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn log_file_append_preserves_content() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("log.txt");
        fs::write(&path, b"hello").unwrap();
        {
            let mut f = open_log_file_secure_append(&path).unwrap();
            f.write_all(b" world").unwrap();
        }
        let got = fs::read(&path).unwrap();
        assert_eq!(&got, b"hello world");
    }

    #[test]
    fn atomic_config_write_creates_file() {
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("config.xml");
        write_config_secure_new_0600(&cfg, b"<x/>").unwrap();
        let contents = fs::read(&cfg).unwrap();
        assert_eq!(contents, b"<x/>");
        for entry in fs::read_dir(dir.path()).unwrap() {
            let p = entry.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy();
            assert!(!name.starts_with(".aria_move.config.tmp."), "leftover temp file: {}", name);
        }
        // Ensure file isn't read-only (best-effort check)
        if let Some(wide) = to_wide(&cfg) {
            unsafe {
                let attrs = GetFileAttributesW(wide.as_ptr());
                assert_ne!(attrs, u32::MAX);
                assert_eq!(attrs & FILE_ATTRIBUTE_READONLY, 0, "config file unexpectedly read-only");
            }
        }
    }

    #[test]
    fn tmp_name_uniqueness() {
        let target = Path::new("dummy.xml");
        let a = tmp_sibling_name(target);
        let b = tmp_sibling_name(target);
        assert_ne!(a, b);
    }

    #[test]
    fn disk_space_smoke() {
        let dir = tempdir().unwrap();
        let bytes = check_disk_space(dir.path()).unwrap();
        assert!(bytes > 0);
    }
}
