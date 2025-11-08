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
use std::time::Instant;
#[cfg(windows)]
use std::time::Duration;
#[cfg(windows)]
use tracing::warn;
use tracing::trace;

#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::CloseHandle,
    Storage::FileSystem::{CreateFileW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN, FILE_ATTRIBUTE_NORMAL, OPEN_ALWAYS},
};

#[cfg(windows)]
const GENERIC_READ: u32 = 0x8000_0000;
#[cfg(windows)]
const GENERIC_WRITE: u32 = 0x4000_0000;

/// RAII guard held while a directory-level lock is active.
/// Public for integration tests / advanced callers; stability not guaranteed.
pub struct DirLock {
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
        // Try to remove the on-disk lock file name when dropping the guard so that
        // completed operations don't leave stale `.aria_move.dir.lock` files behind.
        // This is best-effort: removal may fail if other processes still hold or have
        // recreated the lock file; ignore errors.
        let _ = std::fs::remove_file(&self._path);
    }
}

fn lock_file_path(dir: &Path) -> PathBuf {
    dir.join(".aria_move.dir.lock")
}

/// Acquire an exclusive lock for `dir` by opening/locking a sidecar lock file.
/// Blocks until acquired. Returns a guard that releases on drop.
/// Blocking acquire of a directory lock. Waits until the lock is available.
pub fn acquire_dir_lock(dir: &Path) -> io::Result<DirLock> {
    let lock_path = lock_file_path(dir);
    let start = Instant::now();

    #[cfg(unix)]
    {
        // Create or open the lock file with restrictive permissions on first creation.
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(libc::O_CLOEXEC)
            .mode(0o600)
            .open(&lock_path)?;

        // Block until an exclusive lock is acquired.
        let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) };
        if rc != 0 {
            return Err(io::Error::last_os_error());
        }
        let waited = start.elapsed();
        if waited.is_zero() {
            trace!(path = %lock_path.display(), "lock acquired immediately");
        } else {
            trace!(path = %lock_path.display(), waited_ms = waited.as_millis() as u64, "lock acquired after wait");
        }
    Ok(DirLock { file: f, _path: lock_path })
    }

    #[cfg(windows)]
    {
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

        let mut attempts: u32 = 0;
        loop {
            let handle = unsafe {
                CreateFileW(
                    wide.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    0,                // no sharing => exclusive
                    std::ptr::null_mut(),
                    OPEN_ALWAYS,
                    FILE_ATTRIBUTE_NORMAL,
                    std::ptr::null_mut(),
                )
            };

            if handle as isize != -1 {
                // Ensure the on-disk lock file is hidden so casual dir listings don't show it.
                let _ = unsafe { SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_NORMAL | FILE_ATTRIBUTE_HIDDEN) };
                let waited = start.elapsed();
                trace!(path = %lock_path.display(), attempts = attempts, waited_ms = waited.as_millis() as u64, "lock acquired");
                return Ok(DirLock { handle: handle as isize, _path: lock_path.clone() });
            }

            let err = io::Error::last_os_error();
            // ERROR_SHARING_VIOLATION = 32 -> retry until available
            if let Some(code) = err.raw_os_error() {
                if code == 32 {
                    attempts += 1;
                    if attempts % 10 == 0 {
                        warn!(path = %lock_path.display(), attempts = attempts, "still waiting for directory lock");
                    }
                    sleep(Duration::from_millis(50));
                    continue;
                }
            }
            // Unexpected error
            return Err(err);
        }
    }
}

/// Try to acquire an exclusive lock for `dir` without blocking.
/// Returns Ok(Some(DirLock)) on success, Ok(None) if another process holds the lock,
/// or Err on unexpected errors.
/// Non-blocking attempt to acquire a directory lock.
/// Returns Ok(None) if lock is currently held elsewhere.
pub fn try_acquire_dir_lock(dir: &Path) -> io::Result<Option<DirLock>> {
    let lock_path = lock_file_path(dir);
    let start = Instant::now();

    #[cfg(unix)]
    {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(libc::O_CLOEXEC)
            .mode(0o600)
            .open(&lock_path)?;

        let rc = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if rc == 0 {
            trace!(path = %lock_path.display(), waited_ms = start.elapsed().as_millis() as u64, "try-lock success");
            return Ok(Some(DirLock { file: f, _path: lock_path }));
        }
        let err = io::Error::last_os_error();
        if let Some(code) = err.raw_os_error() && code == libc::EWOULDBLOCK {
            trace!(path = %lock_path.display(), "try-lock would block");
            return Ok(None);
        }
    Err(err)
    }

    #[cfg(windows)]
    {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;

        let wide: Vec<u16> = lock_path
            .as_os_str()
            .encode_wide()
            .chain(once(0))
            .collect();

        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                0,
                std::ptr::null_mut(),
                OPEN_ALWAYS,
                FILE_ATTRIBUTE_NORMAL,
                std::ptr::null_mut(),
            )
        };
        if handle as isize != -1 {
            // Mark the lock file hidden to avoid cluttering directories.
            let _ = unsafe { SetFileAttributesW(wide.as_ptr(), FILE_ATTRIBUTE_NORMAL | FILE_ATTRIBUTE_HIDDEN) };
            trace!(path = %lock_path.display(), waited_ms = start.elapsed().as_millis() as u64, "try-lock success");
            return Ok(Some(DirLock { handle: handle as isize, _path: lock_path }));
        }
        let err = io::Error::last_os_error();
        if let Some(code) = err.raw_os_error() {
            // ERROR_SHARING_VIOLATION => already locked
            if code == 32 {
                trace!(path = %lock_path.display(), "try-lock would block");
                return Ok(None);
            }
        }
        Err(err)
    }
}

/// Acquire a move lock for `src` by locking its parent directory.
/// Serializes operations on the same source path.
/// Acquire a move lock for a source path (locks its parent directory).
pub fn acquire_move_lock(src: &Path) -> io::Result<DirLock> {
    let parent = src.parent().unwrap_or_else(|| Path::new("."));
    acquire_dir_lock(parent)
}
