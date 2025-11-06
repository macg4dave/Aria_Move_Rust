//! Advisory move lock.
//! Uses a sidecar lock file to ensure only one process operates in a directory at a time.
//!
//! Design:
//! - We lock by opening/holding a file `.aria_move.dir.lock` inside the target directory.
//! - Unix: use flock(LOCK_EX) on the file descriptor (blocks until acquired).
//! - Windows: open the file without sharing (exclusive); retry on sharing violations.
//!
//! Notes:
//! - The lock is released when the DirLock guard is dropped.
//! - This module returns io::Result to keep low-level errors precise.
//!
//! Callers typically use:
//!   - acquire_move_lock(src_path)       // serialize per-source (parent dir)
//!   - acquire_dir_lock(destination_dir) // serialize finalization into destination

use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Storage::FileSystem::{CreateFileW, FILE_ATTRIBUTE_NORMAL, GENERIC_READ, GENERIC_WRITE, OPEN_ALWAYS},
};

/// RAII guard held while a directory-level lock is active.
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
            // Unlock by closing; flock releases on fd close. Best-effort: ignore errors.
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

/// Acquire an exclusive lock for `dir` by opening/locking a sidecar lock file.
/// Blocks until acquired. Returns a guard that releases on drop.
pub(crate) fn acquire_dir_lock(dir: &Path) -> io::Result<DirLock> {
    let lock_path = lock_file_path(dir);

    #[cfg(unix)]
    {
        // Create or open the lock file with restrictive permissions on first creation.
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .open(&lock_path)?;

        // Block until an exclusive lock is acquired.
        let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        return Ok(DirLock { file: f, _path: lock_path });
    }

    #[cfg(windows)]
    {
        use std::ffi::OsStr;
        use std::iter::once;
        use std::os::windows::ffi::OsStrExt;
        use std::thread::sleep;
        use std::time::Duration;

        // Convert Path -> wide string (null-terminated)
        let wide: Vec<u16> = lock_path
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

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
            // ERROR_SHARING_VIOLATION = 32 -> retry until available
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

/// io::Error adapter with context/hints, suitable for `.map_err(...)` in io::Result code.
pub(crate) fn io_error_with_help<'a>(
    action: &'a str,
    path: &'a Path,
) -> impl FnOnce(std::io::Error) -> std::io::Error + 'a {
    let action = action.to_string();
    let path = path.to_path_buf();
    move |err: std::io::Error| {
        let raw = err.raw_os_error().map_or(String::new(), |c| format!(" (os error {})", c));
        let msg = format!("{} '{}': {}{}", action, path.display(), err, raw);
        std::io::Error::new(err.kind(), msg)
    }
}

/// Acquire a move lock for `src` by locking its parent directory.
/// Serializes operations on the same source path.
pub(crate) fn acquire_move_lock(src: &Path) -> io::Result<DirLock> {
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    acquire_dir_lock(parent)
}
