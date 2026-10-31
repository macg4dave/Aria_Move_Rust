//! Windows filesystem adapters. POSIX mode setters do not modify ACLs.
use anyhow::{Result, bail};
use std::fs;
use std::io;
use std::path::Path;
use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
pub fn open_log_file_secure_append(path: &Path) -> io::Result<fs::File> {
    super::filesystem::open_log(path)
}
pub fn write_config_secure_new_0600(path: &Path, contents: &[u8]) -> Result<()> {
    super::filesystem::write_config_new(path, contents)
}
pub fn set_dir_mode_0700(_path: &Path) -> io::Result<()> {
    Ok(())
}
pub fn set_file_mode_0600(_path: &Path) -> io::Result<()> {
    Ok(())
}
pub fn ensure_secure_directory(path: &Path, label: &str) -> Result<()> {
    crate::config::paths::reject_symlinks(path)?;
    let meta = fs::metadata(path)?;
    if !meta.is_dir() || meta.permissions().readonly() {
        bail!("{label} '{}' is not a writable directory", path.display());
    }
    Ok(())
}
pub fn check_disk_space(path: &std::path::Path) -> std::io::Result<u64> {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = OsStr::new(path).encode_wide().chain(once(0)).collect();
    if wide[..wide.len() - 1].contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    let mut free_avail: u64 = 0;
    let mut _total: u64 = 0;
    let mut _total_free: u64 = 0;
    // SAFETY: wide is NUL-terminated; all output pointers are valid and exclusive.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_avail,
            &mut _total,
            &mut _total_free,
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(free_avail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
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
            assert!(
                !name.starts_with(".aria_move.config.tmp."),
                "leftover temp file: {}",
                name
            );
        }
    }

    #[test]
    fn disk_space_smoke() {
        let dir = tempdir().unwrap();
        let bytes = check_disk_space(dir.path()).unwrap();
        assert!(bytes > 0);
    }
}
