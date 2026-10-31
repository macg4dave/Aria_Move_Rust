//! Cancellable source locks. A lock failure is an error, never a silent
//! downgrade; `disable_locks` is the explicit opt-out.
use crate::{platform, shutdown};
use anyhow::Result;
use std::fs::File;
use std::path::Path;
use std::time::Duration;

const RETRY_INTERVAL: Duration = Duration::from_millis(25);

/// An exclusive lock handle, released on drop.
#[derive(Debug)]
pub struct SourceLock {
    _handle: File,
}

/// Serialize duplicate calls for one download, without blocking its siblings.
pub fn acquire_source_lock(src: &Path) -> Result<SourceLock> {
    tracing::debug!(path = %src.display(), "Waiting for source lock");
    loop {
        shutdown::check()?;
        match platform::try_lock_source(src) {
            Ok(Some(handle)) => return Ok(SourceLock { _handle: handle }),
            Ok(None) => std::thread::sleep(RETRY_INTERVAL),
            Err(error) => return Err(crate::errors::source_error(src, error)
                .context("source locking failed; use --disable-locks only on filesystems without lock support")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AriaMoveError;

    #[test]
    fn cancellation_interrupts_a_contended_wait() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        let held = acquire_source_lock(temp.path())?;
        let path = temp.path().to_path_buf();
        let waiter = std::thread::spawn(move || acquire_source_lock(&path).map(drop));
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
        let held = acquire_source_lock(temp.path())?;
        let path = temp.path().to_path_buf();
        let waiter = std::thread::spawn(move || acquire_source_lock(&path).map(drop));
        std::thread::sleep(Duration::from_millis(100));
        drop(held);
        waiter.join().unwrap()?;
        Ok(())
    }

    #[test]
    fn source_locks_do_not_lock_siblings_or_parents() -> Result<()> {
        let _guard = shutdown::test_guard();
        let temp = tempfile::tempdir()?;
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        std::fs::write(&first, b"a")?;
        std::fs::write(&second, b"b")?;
        let held = acquire_source_lock(&first)?;
        assert!(platform::try_lock_source(&first)?.is_none());
        assert!(platform::try_lock_source(&second)?.is_some());
        assert!(platform::try_lock_source(temp.path())?.is_some());
        drop(held);
        assert!(platform::try_lock_source(&first)?.is_some());
        Ok(())
    }
}
