//! Shared Unix adapters for Linux and macOS.
use anyhow::Result;
use std::fs::{self, File};
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
pub fn open_log_file_secure_append(path: &Path) -> io::Result<File> {
    super::filesystem::open_log(path)
}
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    super::filesystem::write_config_new(path, contents)
}
pub fn set_dir_mode_0700(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}
pub fn set_file_mode_0600(path: &Path) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
}
pub fn check_disk_space(path: &Path) -> io::Result<u64> {
    use std::os::unix::ffi::OsStrExt;
    let path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: path is a valid C string; stat is writable and initialized only on success.
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: statvfs succeeded and initialized the structure.
    let stat = unsafe { stat.assume_init() };
    let size = if stat.f_frsize == 0 {
        stat.f_bsize
    } else {
        stat.f_frsize
    };
    Ok((stat.f_bavail as u128)
        .saturating_mul(size as u128)
        .min(u64::MAX as u128) as u64)
}
#[cfg(test)]
mod unix_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::tempdir;

    #[test]
    fn preserve_existing_log_file_mode() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("log.txt");
        fs::write(&path, b"hello").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let _f = open_log_file_secure_append(&path).unwrap();
        // Mode should remain 0640 (not forced to 0600) because file pre-existed.
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640, "existing permissions should be preserved");
    }

    #[test]
    fn new_log_file_gets_0600() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("new_log.txt");
        assert!(!path.exists());
        let _f = open_log_file_secure_append(&path).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "newly created log file should be 0600");
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
            assert!(
                !name.starts_with(".aria_move.config.tmp."),
                "leftover temp file: {}",
                name
            );
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

#[cfg(test)]
mod macos_tests {
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
        assert!(write_config_secure_new_0600(&cfg, b"<b/>").is_err());
        let contents = fs::read(&cfg).unwrap();
        assert_eq!(contents, b"<a/>");
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
