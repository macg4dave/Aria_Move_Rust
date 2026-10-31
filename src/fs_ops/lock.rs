//! Cancellable directory locks. Lock failures never silently disable protection.
use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// An exclusive directory lock, released when the guard is dropped.
pub struct DirLock {
    _file: File,
}

/// Wait for a directory lock, checking cancellation between nonblocking attempts.
pub fn acquire_dir_lock(dir: &Path) -> io::Result<DirLock> {
    tracing::debug!(path = %dir.display(), "Waiting for directory lock");
    loop {
        if crate::shutdown::is_requested() {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                crate::AriaMoveError::Interrupted,
            ));
        }
        if let Some(lock) = try_acquire_dir_lock(dir)? {
            return Ok(lock);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Attempt an exclusive lock without waiting. Unix locks the directory inode;
/// Windows holds an exclusive, delete-on-close sidecar handle.
pub fn try_acquire_dir_lock(dir: &Path) -> io::Result<Option<DirLock>> {
    crate::config::paths::reject_symlinks(dir)?;
    #[cfg(unix)]
    {
        use std::os::{fd::AsRawFd, unix::fs::OpenOptionsExt};
        let file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(dir)?;
        // SAFETY: file owns a valid directory fd. Nonblocking flock has no borrowed pointers.
        if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
            return Ok(Some(DirLock { _file: file }));
        }
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            Ok(None)
        } else {
            Err(error)
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_HIDDEN, FILE_FLAG_DELETE_ON_CLOSE, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        let path = dir.join(".aria_move.dir.lock");
        crate::config::paths::reject_symlinks(&path)?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .access_mode(0x8000_0000 | 0x4000_0000 | 0x0001_0000) // READ | WRITE | DELETE
            .share_mode(0)
            .custom_flags(
                FILE_ATTRIBUTE_HIDDEN | FILE_FLAG_DELETE_ON_CLOSE | FILE_FLAG_OPEN_REPARSE_POINT,
            )
            .open(&path);
        match file {
            Ok(file) => {
                if crate::config::paths::is_link(&file.metadata()?) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "lock is a reparse point",
                    ));
                }
                Ok(Some(DirLock { _file: file }))
            }
            Err(e) if e.raw_os_error() == Some(32) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

/// Lock the source's parent directory.
pub fn acquire_move_lock(src: &Path) -> io::Result<DirLock> {
    acquire_dir_lock(
        src.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")),
    )
}

/// Use one deterministic order for all operations, including reversed base pairs.
/// Deduplication also avoids self-deadlocking when both parents are the same inode.
pub(super) fn acquire_move_locks(
    src: &Path,
    destination: &Path,
    disabled: bool,
) -> crate::AMResult<Vec<DirLock>> {
    if disabled {
        return Ok(Vec::new());
    }
    let parent = src
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut dirs: Vec<PathBuf> = vec![
        dunce::canonicalize(parent)?,
        dunce::canonicalize(destination)?,
    ];
    dirs.sort();
    dirs.dedup();
    let mut locks = Vec::new();
    for dir in dirs {
        crate::shutdown::check()?;
        match acquire_dir_lock(&dir) {
            Ok(lock) => locks.push(lock),
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {
                return Err(crate::AriaMoveError::Interrupted.into());
            }
            Err(e) => return Err(super::helpers::io_error_with_help("acquire lock", &dir)(e)),
        }
    }
    Ok(locks)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_interrupts_a_contended_lock() -> crate::AMResult<()> {
        let _guard = crate::shutdown::test_guard();
        let td = tempfile::tempdir()?;
        let held = acquire_dir_lock(td.path())?;
        let path = td.path().to_path_buf();
        let (sender, receiver) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            let result = acquire_dir_lock(&path).map(|_| ()).map_err(|e| e.kind());
            sender.send(result).unwrap();
        });
        crate::shutdown::request();
        let result = receiver.recv_timeout(Duration::from_secs(3));
        drop(held);
        worker.join().unwrap();
        assert_eq!(result?, Err(io::ErrorKind::Interrupted));
        Ok(())
    }
}
