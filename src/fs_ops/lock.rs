//! Cancellable directory locks. A lock failure is an error, never a silent
//! downgrade; `disable_locks` is the explicit opt-out.
use crate::{platform, shutdown};
use anyhow::Result;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

const RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// An exclusive directory lock, released on drop.
#[derive(Debug)]
pub struct DirLock {
    _handle: File,
}

/// Wait for an exclusive lock on `dir`, polling so cancellation is honoured.
pub fn acquire_dir_lock(dir: &Path) -> Result<DirLock> {
    tracing::debug!(path = %dir.display(), "Waiting for directory lock");
    loop {
        shutdown::check()?;
        match platform::try_lock_directory(dir) {
            Ok(Some(handle)) => return Ok(DirLock { _handle: handle }),
            Ok(None) => std::thread::sleep(RETRY_INTERVAL),
            Err(error) => {
                let denied = error.kind() == io::ErrorKind::PermissionDenied;
                let error = crate::errors::io_error("lock directory", dir, error);
                return Err(if denied {
                    error.context("directory locking failed; on ZFS/NFS or container mounts that reject locks, use --disable-locks")
                } else {
                    error
                });
            }
        }
    }
}

/// Lock the source's parent and the destination directory, in one global
/// order so concurrent movers cannot deadlock; a shared directory is locked once.
pub(super) fn acquire_move_locks(
    src: &Path,
    dest_dir: &Path,
    disabled: bool,
) -> Result<Vec<DirLock>> {
    if disabled {
        return Ok(Vec::new());
    }
    let mut dirs: Vec<PathBuf> = [platform::parent_dir(src), dest_dir]
        .into_iter()
        .map(dunce::canonicalize)
        .collect::<io::Result<_>>()?;
    dirs.sort();
    dirs.dedup();
    dirs.iter().map(|dir| acquire_dir_lock(dir)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AriaMoveError;

    #[test]
    fn cancellation_interrupts_a_contended_wait() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        let held = acquire_dir_lock(temp.path())?;
        let path = temp.path().to_path_buf();
        let waiter = std::thread::spawn(move || acquire_dir_lock(&path).map(drop));
        std::thread::sleep(Duration::from_millis(100));
        assert!(!waiter.is_finished(), "must wait while the lock is held");
        shutdown::request();
        let error = waiter.join().unwrap().unwrap_err();
        drop(held);
        assert_eq!(
            error.downcast_ref::<AriaMoveError>(),
            Some(&AriaMoveError::Interrupted)
        );
        Ok(())
    }

    #[test]
    fn a_waiter_proceeds_once_the_holder_releases() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        let held = acquire_dir_lock(temp.path())?;
        let path = temp.path().to_path_buf();
        let waiter = std::thread::spawn(move || acquire_dir_lock(&path).map(drop));
        std::thread::sleep(Duration::from_millis(100));
        drop(held);
        waiter.join().unwrap()?;
        Ok(())
    }

    #[test]
    fn move_locks_cover_both_directories_once() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        let (incoming, completed) = (temp.path().join("in"), temp.path().join("out"));
        std::fs::create_dir(&incoming)?;
        std::fs::create_dir(&completed)?;
        let locks = acquire_move_locks(&incoming.join("file"), &completed, false)?;
        assert_eq!(locks.len(), 2);
        assert!(platform::try_lock_directory(&incoming)?.is_none());
        assert!(platform::try_lock_directory(&completed)?.is_none());
        drop(locks);
        let same = acquire_move_locks(&completed.join("file"), &completed, false)?;
        assert_eq!(same.len(), 1, "a shared directory must not self-deadlock");
        drop(same);
        assert!(acquire_move_locks(&incoming.join("file"), &completed, true)?.is_empty());
        Ok(())
    }
}
