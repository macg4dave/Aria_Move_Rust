//! Advisory move lock.
//! Uses a sidecar lock file and fs2::FileExt to ensure only one process moves a path at a time.

use anyhow::{bail, Result};
use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, GENERIC_READ, GENERIC_WRITE, OPEN_ALWAYS},
};

/// Guard type held while a directory-level lock is held.
/// Kept crate-visible so sibling modules can hold the RAII guard.
pub(crate) struct DirLock {
    #[cfg(unix)]
    file: File,
    #[cfg(windows)]
    handle: isize, // HANDLE
    _path: PathBuf,
}

impl Drop for DirLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            // Unlock by closing; flock releases on fd close.
            // Best-effort; ignore errors on drop.
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        }
        #[cfg(windows)]
        unsafe {
            if self.handle != 0 {
                let _ = CloseHandle(self.handle as _);
            }
        }
    }
}

fn lock_file_path(dir: &Path) -> PathBuf {
    dir.join(".aria_move.dir.lock")
}

/// Acquire an exclusive lock file for `dir`. Blocks until acquired.
pub(crate) fn acquire_dir_lock(dir: &Path) -> io::Result<DirLock> {
    let lock_path = lock_file_path(dir);

    #[cfg(unix)]
    {
        // Open or create the lock file; 0600 perms by default for new files.
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)?;
        // Block until an exclusive lock is acquired.
        let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(DirLock { file: f, _path: lock_path })
    }

    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;
        use std::thread::sleep;
        use std::time::Duration;

        let wide: Vec<u16> = OsStr::new(&lock_path).encode_wide().chain(once(0)).collect();

        loop {
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,                // no sharing => exclusive
                    std::ptr::null_mut(),
                    OPEN_ALWAYS,
                    FILE_ATTRIBUTE_NORMAL,
                    0,
                )
            };

            if handle as isize != -1 {
                return Ok(DirLock { handle: handle as isize, _path: lock_path.clone() });
            }

            let err = io::Error::last_os_error();
            // ERROR_SHARING_VIOLATION = 32; fall back to retry
            if let Some(code) = err.raw_os_error() {
                if code == 32 {
                    sleep(Duration::from_millis(50));
                    continue;
                }
            }
            // Unexpected error
            return Err(err);
        }
    }
}

/// Helper to convert an io::Error into a richer io::Error with context/help text.
///
/// Usage:
///     .map_err(io_error_with_help("open lock file", &lock_path))?;
pub(crate) fn io_error_with_help<'a>(
    action: &'a str,
    path: &'a Path,
) -> impl FnOnce(std::io::Error) -> std::io::Error + 'a {
    let action = action.to_string();
    let path = path.to_path_buf();
    move |err: std::io::Error| {
        let raw = err.raw_os_error().map_or("".to_string(), |c| format!(" (os error {})", c));
        let msg = format!("{} '{}': {}{}", action, path.display(), err, raw);
        std::io::Error::new(err.kind(), msg)
    }
}

/// Acquire a move lock for the provided source path by locking its parent directory.
/// This is a small wrapper used by higher-level move logic to serialize claims on a file.
pub(crate) fn acquire_move_lock(src: &Path) -> io::Result<DirLock> {
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    acquire_dir_lock(parent)
}
