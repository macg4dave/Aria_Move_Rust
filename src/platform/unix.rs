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
    let f = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)?;
    // Enforce 0600 even if file pre-existed with different mode.
    let meta = f.metadata();
    if let Ok(m) = meta {
        let current = m.permissions().mode() & 0o777;
        if current != 0o600 {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
    }
    Ok(f)
}

/// Write config atomically: temp file (0600) + fsync + rename + fsync dir.
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config path has no parent"))?;
    fs::create_dir_all(parent).with_context(|| format!("create parent '{}'", parent.display()))?;

    // Hidden sibling temp path (unique) next to target.
    let tmp = tmp_sibling_name(path);

    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .with_context(|| format!("create temp '{}'", tmp.display()))?;
    f.write_all(contents).context("write temp config")?;
    f.sync_all().context("fsync temp config")?;
    drop(f);

    if let Err(e) = fs::rename(&tmp, path) {
        // Best-effort cleanup of temp file on failure.
        let _ = fs::remove_file(&tmp);
        return Err(e).with_context(|| format!("rename '{}' -> '{}'", tmp.display(), path.display()));
    }

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
    use std::time::{SystemTime, UNIX_EPOCH};
    let pid = std::process::id();
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    // Add a simple counter to reduce collision risk in ultra-fast successive calls.
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let name = format!(".aria_move.config.tmp.{pid}.{nanos}.{seq}");
    target.parent().unwrap_or_else(|| Path::new(".")).join(name)
}

/// Check available disk space at the given path (returns bytes available).
/// Uses statvfs on Unix. Returns Ok(available_bytes) or an IO error.
pub fn check_disk_space(path: &Path) -> io::Result<u64> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::ffi::CString;
        use std::mem::MaybeUninit;
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains null byte"))?;
        unsafe {
            let mut stat: MaybeUninit<libc::statvfs> = MaybeUninit::uninit();
            if libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) != 0 {
                return Err(io::Error::last_os_error());
            }
            let stat = stat.assume_init();
            Ok((stat.f_bavail as u64).saturating_mul(stat.f_bsize))
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        Ok(u64::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn enforce_log_file_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("log.txt");
        // Pre-create with loose perms.
        fs::write(&path, b"hello").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let _f = open_log_file_secure_append(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn atomic_config_write_sets_mode_and_no_temp_leftover() {
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("config.xml");
        write_config_secure_new_0600(&cfg, b"<x/>").unwrap();
        let contents = fs::read(&cfg).unwrap();
        assert_eq!(contents, b"<x/>");
        let mode = fs::metadata(&cfg).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        // Ensure no leftover temp files.
        for entry in fs::read_dir(dir.path()).unwrap() {
            let p = entry.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy();
            assert!(!name.starts_with(".aria_move.config.tmp."), "leftover temp file: {}", name);
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
