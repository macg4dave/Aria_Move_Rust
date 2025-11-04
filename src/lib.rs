//! aria_move library — modular entry points.
pub mod config;
pub mod fs_ops;
pub mod utils;

// re-export the public API from modules at the crate root
pub use config::{Config, LogLevel, ensure_default_config_exists, default_config_path};
pub use fs_ops::{move_dir, move_entry, move_file, resolve_source_path, validate_paths};
