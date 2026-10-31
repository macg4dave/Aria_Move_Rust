//! Moving completed downloads: source resolution, locking, exclusive rename
//! and a verified copy fallback across filesystems.
mod lock;
mod metadata;
mod mover;
mod naming;
mod resolve;
mod space;
mod transfer;

pub use lock::{DirLock, acquire_dir_lock};
pub use mover::move_entry;
pub use resolve::resolve_source_path;
