//! Global shutdown flag set by ctrlc handler for SIGINT/SIGTERM.
//! Other modules can call is_requested() to abort promptly.
//! Process-wide shutdown coordination.
//! Provides a flag set by signal handlers so long-running operations can exit early.
//! 
use std::sync::atomic::{AtomicBool, Ordering};

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub fn request() {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

pub fn is_requested() -> bool {
    SHUTDOWN.load(Ordering::SeqCst)
}

#[allow(dead_code)]
pub fn reset() {
    SHUTDOWN.store(false, Ordering::SeqCst);
}
