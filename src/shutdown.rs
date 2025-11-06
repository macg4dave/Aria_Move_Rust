//! Global shutdown flag set by ctrlc handler for SIGINT/SIGTERM.
//! Other modules can call is_requested() to abort promptly.
//! Process-wide shutdown coordination.
//! Provides a flag set by signal handlers so long-running operations can exit early.
//!
//! Notes:
//! - Relaxed atomics are sufficient for a one-way "stop" flag.
//! - `request()` is safe to call from signal handlers.
//!
use std::sync::atomic::{AtomicBool, Ordering};

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

/// Request a cooperative shutdown (idempotent).
#[inline]
pub fn request() {
    SHUTDOWN.store(true, Ordering::Relaxed);
}

/// Check whether a shutdown has been requested.
#[inline]
pub fn is_requested() -> bool {
    SHUTDOWN.load(Ordering::Relaxed)
}

/// Test/utility-only: clear the shutdown flag.
#[cfg(any(test, feature = "test-utils"))]
#[inline]
pub fn reset() {
    SHUTDOWN.store(false, Ordering::Relaxed);
}
