//! Library crate root for aria_move.
//! Exposes the public API by organizing modules and re-exporting key functions/types.
//!
//! Notes:
//! - Re-exports come from concrete submodules to avoid accidental breakage if mod.rs changes.
//! - Keep this surface minimal and stable.

pub mod config;
pub mod errors;
pub mod fs_ops;
pub mod platform;
pub mod shutdown;
pub mod utils;

// Public API (stable)
// Config
pub use config::paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
pub use config::types::{Config, LogLevel};
pub use config::xml::{ensure_default_config_exists, load_config_from_xml};

// Operations
pub use fs_ops::{move_dir, move_entry, move_file, resolve_source_path, safe_copy_and_rename};

// Errors
pub use errors::AriaMoveError;
