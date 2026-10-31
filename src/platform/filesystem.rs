//! Shared filesystem policy; native calls never replace another destination.
use crate::config::paths::{create_directory, reject_symlinks};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

pub(crate) fn rename_noreplace(src: &Path, dst: &Path) -> io::Result<()> {
    reject_symlinks(src)?;
    // Existing leaf entries (including dangling links) must yield AlreadyExists.
    reject_symlinks(dst.parent().unwrap_or_else(|| Path::new(".")))?;
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{MOVEFILE_WRITE_THROUGH, MoveFileExW};
        let from: Vec<u16> = src.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = dst.as_os_str().encode_wide().chain(Some(0)).collect();
        if from[..from.len() - 1].contains(&0) || to[..to.len() - 1].contains(&0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path contains NUL",
            ));
        }
        // SAFETY: both buffers are NUL-terminated and live for the call. No
        // REPLACE_EXISTING or COPY_ALLOWED flag is supplied, for files or trees.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(src.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(dst.as_os_str().as_bytes())?;
        #[cfg(target_os = "linux")]
        // SAFETY: valid C strings, AT_FDCWD uses process cwd, no pointer outlives call.
        let rc = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        // SAFETY: valid C strings, exclusive rename flag prevents replacement.
        let rc = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        return Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "exclusive rename unavailable on this target",
        ));
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        if rc == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

/// Read a regular file without following a leaf link. Windows denies concurrent
/// writes and deletion for the lifetime of this handle.
pub(crate) fn open_regular_read(path: &Path) -> io::Result<File> {
    reject_symlinks(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .share_mode(1); // FILE_SHARE_READ
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || crate::config::paths::is_link(&meta) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source is not a regular file",
        ));
    }
    Ok(file)
}

pub(crate) fn open_directory(path: &Path) -> io::Result<File> {
    reject_symlinks(path)?;
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        options
            .access_mode(0x80)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_dir() || crate::config::paths::is_link(&metadata) {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "expected directory",
        ));
    }
    Ok(file)
}

pub(crate) fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        open_directory(path)?.sync_all()
    }
    #[cfg(windows)]
    {
        let _ = path;
        Ok(())
    } // Windows publication uses MOVEFILE_WRITE_THROUGH.
}

pub(crate) fn write_config_new(path: &Path, contents: &[u8]) -> anyhow::Result<()> {
    reject_symlinks(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_directory(parent)?;
    let mut stage = tempfile::Builder::new()
        .prefix(".aria_move.config.")
        .tempfile_in(parent)?;
    stage.write_all(contents)?;
    stage.as_file().sync_all()?;
    reject_symlinks(parent)?;
    rename_noreplace(stage.path(), path)?;
    // Temp ownership guards remove only their own old name; publication is never rolled back.
    sync_directory(parent)?;
    Ok(())
}

pub(crate) fn open_log(path: &Path) -> io::Result<File> {
    reject_symlinks(path)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    create_directory(parent)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() || crate::config::paths::is_link(&meta) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "log is not a regular file",
        ));
    }
    Ok(file)
}
