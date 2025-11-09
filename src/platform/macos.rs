//! macOS implementations of platform helpers.
//! Split from the generic Unix module to allow future macOS-specific extensions.

use super::common_unix::atomic_write_0600;
use anyhow::Result;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;

/// Open log file for appending. Set 0600 only when creating a new file; preserve existing mode.
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let existed = path.exists();
    let f = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600) // applied on create
        .open(path)?;
    if !existed {
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    Ok(f)
}

/// Write config atomically: temp file (0600) + fsync + rename + fsync dir.
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    atomic_write_0600(path, contents)
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
#[cfg(test)]
fn tmp_sibling_name(target: &Path) -> std::path::PathBuf {
    super::temp::tmp_config_sibling_name(target)
}

/// Check available disk space at the given path (returns bytes available) using statvfs.
pub fn check_disk_space(path: &Path) -> io::Result<u64> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn preserve_existing_log_file_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("log.txt");
        fs::write(&path, b"hello").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let _f = open_log_file_secure_append(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[test]
    fn new_log_file_gets_0600() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("new_log.txt");
        assert!(!path.exists());
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
        for entry in fs::read_dir(dir.path()).unwrap() {
            let p = entry.unwrap().path();
            let name = p.file_name().unwrap().to_string_lossy();
            assert!(
                !name.starts_with(".aria_move.config.tmp."),
                "leftover temp file: {}",
                name
            );
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
    fn tmp_names_unique_under_concurrency() {
        use std::sync::Mutex;
        use std::thread;
        let target = Path::new("dummy.xml");
        let names = Mutex::new(Vec::new());
        let mut threads = Vec::new();
        for _ in 0..32 {
            let t = target.to_path_buf();
            threads.push(thread::spawn(move || tmp_sibling_name(&t)));
        }
        for th in threads {
            names.lock().unwrap().push(th.join().unwrap());
        }
        let v = names.lock().unwrap();
        let mut sorted = v.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), v.len(), "duplicate temp names found");
    }

    #[test]
    fn disk_space_nonexistent_path_errors() {
        let p = Path::new("/this/definitely/does/not/exist/aria_move_test");
        let err = check_disk_space(p).unwrap_err();
        let s = format!("{}", err);
        assert!(s.contains("No such file") || s.contains("no such file"));
    }

    #[test]
    fn disk_space_smoke() {
        let dir = tempdir().unwrap();
        let bytes = check_disk_space(dir.path()).unwrap();
        assert!(bytes > 0);
    }

    #[test]
    fn config_write_second_call_conflict() {
        let dir = tempdir().unwrap();
        let cfg = dir.path().join("config.xml");
        write_config_secure_new_0600(&cfg, b"<a/>").unwrap();
        // Second write should succeed and replace contents atomically.
        write_config_secure_new_0600(&cfg, b"<b/>").unwrap();
        let contents = fs::read(&cfg).unwrap();
        assert_eq!(contents, b"<b/>");
        let mode = fs::metadata(&cfg).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        // Ensure no temp leftovers after two writes.
        for entry in fs::read_dir(dir.path()).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(
                !name.starts_with(".aria_move.config.tmp."),
                "leftover temp file after double write: {name}"
            );
        }
    }

    #[test]
    fn write_config_rename_failure_cleans_temp() {
        let dir = tempdir().unwrap();
        // Create a directory where file is expected to trigger rename error.
        let cfg_dir = dir.path().join("config.xml");
        fs::create_dir(&cfg_dir).unwrap();
        let res = write_config_secure_new_0600(&cfg_dir, b"<x/>");
        assert!(res.is_err(), "expected error writing to directory path");
        // Ensure temp cleaned up.
        for entry in fs::read_dir(dir.path()).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().into_owned();
            assert!(
                !name.starts_with(".aria_move.config.tmp."),
                "leftover temp after failed rename: {name}"
            );
        }
    }

    #[test]
    fn concurrent_log_appends() {
        use std::sync::Arc;
        use std::thread;
        let dir = tempdir().unwrap();
        let log_path = dir.path().join("log.txt");
        let path_arc = Arc::new(log_path);
        let mut threads = Vec::new();
        for i in 0..8 {
            let p = path_arc.clone();
            threads.push(thread::spawn(move || {
                let mut f = open_log_file_secure_append(&p).unwrap();
                f.write_all(format!("line{i}\n").as_bytes()).unwrap();
            }));
        }
        for t in threads {
            t.join().unwrap();
        }
        let contents = fs::read(path_arc.as_ref()).unwrap();
        let s = String::from_utf8(contents).unwrap();
        for i in 0..8 {
            assert!(s.contains(&format!("line{i}")), "missing line{i} in log");
        }
        let mode = fs::metadata(path_arc.as_ref())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "log file mode drifted from 0600");
    }
}
