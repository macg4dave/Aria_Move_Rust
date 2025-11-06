//! Filesystem operations: modularized. Re-exports the public surface.

mod atomic;
mod claim;
mod copy;
mod dir_move;
mod entry;
mod file_move;
mod io_copy;
mod lock;
mod metadata;
mod resolve;
mod space;
mod util;

// Public API (stable)
pub use dir_move::move_dir;
pub use entry::move_entry;
pub use file_move::move_file;
pub use resolve::resolve_source_path;
pub use copy::{safe_copy_and_rename, safe_copy_and_rename_with_metadata};
