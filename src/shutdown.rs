//! Process-wide cooperative cancellation.
//!
//! The Ctrl-C/SIGTERM handler calls [`request`]; long-running operations call
//! [`check`] at safe points and unwind with [`AriaMoveError::Interrupted`].
//! A relaxed atomic is sufficient for a one-way stop flag.

use crate::AriaMoveError;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request cancellation. Safe to call from a signal handler thread.
pub fn request() {
    REQUESTED.store(true, Ordering::Relaxed);
}

/// Whether cancellation has been requested.
pub fn is_requested() -> bool {
    REQUESTED.load(Ordering::Relaxed)
}

/// Abort at a cancellation point.
pub fn check() -> crate::AMResult<()> {
    if is_requested() {
        Err(AriaMoveError::Interrupted.into())
    } else {
        Ok(())
    }
}

/// Sleep, then check for cancellation.
pub(crate) fn pause(duration: Duration) -> crate::AMResult<()> {
    std::thread::sleep(duration);
    check()
}

/// Serializes unit tests that observe or set the global flag, and clears it on
/// both entry and exit.
#[cfg(test)]
pub(crate) fn test_guard() -> impl Drop {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    struct Guard(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);
    impl Drop for Guard {
        fn drop(&mut self) {
            REQUESTED.store(false, Ordering::Relaxed);
        }
    }
    let lock = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    REQUESTED.store(false, Ordering::Relaxed);
    Guard(lock)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_is_observed_by_check_and_cleared_by_the_guard() {
        {
            let _guard = test_guard();
            assert!(check().is_ok());
            request();
            let error = check().unwrap_err();
            assert_eq!(
                error.downcast_ref::<AriaMoveError>(),
                Some(&AriaMoveError::Interrupted)
            );
        }
        let _guard = test_guard();
        assert!(!is_requested());
    }
}
