//! Unix (non-macOS) implementations of platform helpers.

use anyhow::Result;
use std::fs::{self, File, OpenOptions};
use std::io::{self};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use super::common_unix::atomic_write_0600;

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
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> { atomic_write_0600(path, contents) }

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

// (No local tmp_sibling_name wrapper needed; macOS/windows modules keep theirs if required.)

/// Check available disk space at the given path (returns bytes available).
/// Uses statvfs on Unix. Returns Ok(available_bytes) or an IO error.
pub fn check_disk_space(path: &Path) -> io::Result<u64> {
    #[cfg(target_os = "linux")]
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
            Ok((stat.f_bavail).saturating_mul(stat.f_bsize))
        }
    }
    #[cfg(not(target_os = "linux"))]
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

    // tmp_sibling_name uniqueness test not needed here after removal.

    #[test]
    fn disk_space_smoke() {
        let dir = tempdir().unwrap();
        let bytes = check_disk_space(dir.path()).unwrap();
        assert!(bytes > 0);
    }
}
