//! Filesystem operations (modularized).
//! High-level entry points for moving files/directories, plus supporting utilities.

mod resolve;
mod entry;
mod file_move;
mod dir_move;
mod atomic;
mod copy;
mod meta;
mod lock;
mod helpers;
mod disk;

pub use copy::{safe_copy_and_rename, safe_copy_and_rename_with_metadata};
pub use dir_move::move_dir;
pub use entry::move_entry;
pub use file_move::move_file;
pub use resolve::resolve_source_path;

use crate::config::Config;
use anyhow::Result;

/// Validate the configured paths (wrapper used by CLI).
pub fn validate_paths(cfg: &Config) -> Result<()> {
    cfg.validate()
}
