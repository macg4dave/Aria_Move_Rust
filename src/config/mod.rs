//! Config module (modularized).
//! Provides configuration types, default paths, XML loading, and validation.
//! Re-exports preserve the previous public API for external callers.

pub mod types;
pub mod paths;
pub mod xml;
mod validate;

pub use types::{Config, LogLevel};
pub use paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
pub use xml::{load_config_from_xml, ensure_default_config_exists, create_template_config};

/// Defaults shared across submodules (same values as before).
pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World/incoming";
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
pub const RECENT_FILE_WINDOW: std::time::Duration = std::time::Duration::from_secs(5 * 60);