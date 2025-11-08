//! Global shutdown flag set by ctrlc handler for SIGINT/SIGTERM.
//! Other modules can call is_requested() to abort promptly.
//! Process-wide shutdown coordination.
//! Provides a flag set by signal handlers so long-running operations can exit early.
//!
//! Notes:
//! - Relaxed atomics are sufficient for a one-way "stop" flag.
//! - `request()` is safe to call from signal handlers.
//!
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
// 0 = None, 1 = Signal, 2 = User, 3 = Space, 4 = Error (generic)
static REASON: AtomicU8 = AtomicU8::new(0);

/// Request a cooperative shutdown (idempotent).
#[inline]
pub fn request() {
    request_with_reason(1);
}

/// Request with a semantic reason code; first reason sticks (idempotent flag).
pub fn request_with_reason(code: u8) {
    SHUTDOWN.store(true, Ordering::Relaxed);
    // Preserve first non-zero reason; if already set, leave it.
    if REASON.load(Ordering::Relaxed) == 0 && code != 0 {
        REASON.store(code, Ordering::Relaxed);
    }
}

/// Check whether a shutdown has been requested.
#[inline]
pub fn is_requested() -> bool {
    SHUTDOWN.load(Ordering::Relaxed)
}

/// Get numeric reason code (0 = none).
pub fn reason_code() -> u8 {
    REASON.load(Ordering::Relaxed)
}

/// Test/utility-only: clear the shutdown flag.
#[cfg(any(test, feature = "test-helpers"))]
#[inline]
pub fn reset() {
    SHUTDOWN.store(false, Ordering::Relaxed);
    REASON.store(0, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reason_defaults_to_zero() {
        reset();
        assert_eq!(reason_code(), 0);
        assert!(!is_requested());
    }

    #[test]
    fn request_sets_flag_and_reason() {
        reset();
        request_with_reason(3);
        assert!(is_requested());
        assert_eq!(reason_code(), 3);
    }

    #[test]
    fn first_reason_wins() {
        reset();
        request_with_reason(2);
        request_with_reason(4);
        assert_eq!(reason_code(), 2); // original preserved
    }

    #[test]
    fn reset_clears_both() {
        reset();
        request_with_reason(1);
        reset();
        assert!(!is_requested());
        assert_eq!(reason_code(), 0);
    }
}
