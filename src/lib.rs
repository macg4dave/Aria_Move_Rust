//! aria_move library — modular entry points.
pub mod config;
pub mod fs_ops;
pub mod utils;
pub mod shutdown;

// re-export public API
pub use config::{Config, LogLevel, ensure_default_config_exists, default_config_path, default_log_path, path_has_symlink_ancestor};
pub use fs_ops::{move_dir, move_entry, move_file, resolve_source_path, validate_paths, safe_copy_and_rename};
