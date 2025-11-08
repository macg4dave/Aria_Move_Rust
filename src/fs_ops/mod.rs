//! Filesystem operations
//!
//! This module is a thin façade that:
//! - Declares internal implementation submodules
//! - Re-exports the stable public API used by the rest of the crate
//!
//! Keep this file minimal to avoid name clashes and duplication. Add new public
//! operations by re-exporting them here, not by implementing them inline.

//
// Internal implementation modules (crate-private)
//
mod atomic;
mod claim;
mod copy;
mod dir_move;
mod duplicate;
mod entry;
mod file_move;
mod helpers;
mod io_copy;
mod lock;
mod metadata;
mod resolve;
mod space;
mod util;

//
// Public API (re-exported)
//
pub use atomic::{MoveOutcome, try_atomic_move}; // exposed for targeted tests & outcome usage
pub use copy::{safe_copy_and_rename, safe_copy_and_rename_with_metadata};
pub use dir_move::move_dir;
pub use duplicate::{OnDuplicate, resolve_destination};
pub use entry::move_entry;
pub use file_move::move_file;
pub use helpers::{io_error_with_help, io_error_with_help_io};
pub use metadata::{preserve_metadata, preserve_xattrs};
pub use resolve::resolve_source_path;

// Locking API (currently considered advanced; subject to change)
pub use lock::{DirLock, acquire_dir_lock, acquire_move_lock, try_acquire_dir_lock};
