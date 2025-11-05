//! Filesystem operations: modularized.

mod atomic;
mod copy;
mod dir_move;
mod disk;
mod entry;
mod file_move;
mod helpers;
mod lock;
mod meta;
mod resolve;

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
