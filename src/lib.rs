//! Library crate root for aria_move.
//! Exposes the public API by organizing modules and re-exporting key functions/types.

pub mod config;
pub mod errors;
pub mod fs_ops;
pub mod platform;
pub mod shutdown;
pub mod utils;

// Re-exports (stable public API for binary and tests)
pub use config::{
    default_config_path, default_log_path, ensure_default_config_exists, load_config_from_xml,
    path_has_symlink_ancestor, Config, LogLevel,
};

pub use fs_ops::{
    move_dir, move_entry, move_file, resolve_source_path, safe_copy_and_rename, validate_paths,
};

pub use errors::AriaMoveError;
