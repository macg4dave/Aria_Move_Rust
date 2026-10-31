//! Global shutdown flag set by ctrlc handler for SIGINT/SIGTERM.
//! Other modules can call is_requested() to abort promptly.
//! Process-wide shutdown coordination.
//! Provides a flag set by signal handlers so long-running operations can exit early.
//!
//! Notes:
//! - Relaxed atomics are sufficient for a one-way "stop" flag.
//! - `request()` is safe to call from signal handlers.
//!
use std::sync::atomic::{AtomicU16, Ordering};
// Bit 8 records the request; low byte stores its first nonzero reason atomically.
static STATE: AtomicU16 = AtomicU16::new(0);

/// Request shutdown with the signal reason.
pub fn request() {
    request_with_reason(1);
}
/// Request shutdown; retain the first nonzero reason.
pub fn request_with_reason(code: u8) {
    let _ = STATE.try_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
        Some(0x100 | old | if old & 0xff == 0 { code as u16 } else { 0 })
    });
}
/// Whether cooperative cancellation has been requested.
pub fn is_requested() -> bool {
    STATE.load(Ordering::Relaxed) & 0x100 != 0
}
/// First nonzero reason, or zero if unspecified.
pub fn reason_code() -> u8 {
    STATE.load(Ordering::Relaxed) as u8
}
/// Abort at a cancellation point with the stable typed error.
pub fn check() -> crate::AMResult<()> {
    if is_requested() {
        Err(crate::AriaMoveError::Interrupted.into())
    } else {
        Ok(())
    }
}

/// Test/utility-only: clear the shutdown flag.
#[cfg(any(test, feature = "test-helpers"))]
#[inline]
pub fn reset() {
    STATE.store(0, Ordering::Relaxed);
}

#[cfg(test)]
static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[cfg(test)]
pub(crate) struct TestGuard {
    _lock: std::sync::MutexGuard<'static, ()>,
}
#[cfg(test)]
pub(crate) fn test_guard() -> TestGuard {
    let guard = TestGuard {
        _lock: TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()),
    };
    reset();
    guard
}
#[cfg(test)]
impl Drop for TestGuard {
    fn drop(&mut self) {
        reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_defaults_to_zero() {
        let _guard = crate::shutdown::test_guard();
        reset();
        assert_eq!(reason_code(), 0);
        assert!(!is_requested());
    }

    #[test]
    fn request_sets_flag_and_reason() {
        let _guard = crate::shutdown::test_guard();
        reset();
        request_with_reason(3);
        assert!(is_requested());
        assert_eq!(reason_code(), 3);
        reset();
    }

    #[test]
    fn first_reason_wins() {
        let _guard = crate::shutdown::test_guard();
        reset();
        request_with_reason(2);
        request_with_reason(4);
        assert_eq!(reason_code(), 2); // original preserved
        reset();
    }

    #[test]
    fn reset_clears_both() {
        let _guard = crate::shutdown::test_guard();
        reset();
        request_with_reason(1);
        reset();
        assert!(!is_requested());
        assert_eq!(reason_code(), 0);
    }
}
