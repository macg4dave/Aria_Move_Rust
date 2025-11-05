//! Config module (modularized).
//! Provides configuration types, default paths, XML loading, and validation.
//! Re-exports preserve the previous public API for external callers.

pub mod paths;
pub mod types;
mod validate;
pub mod xml;

pub use paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
pub use types::{Config, LogLevel};
pub use xml::{create_template_config, ensure_default_config_exists, load_config_from_xml};

/// Defaults shared across submodules (same values as before).
pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World/incoming";
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
pub const RECENT_FILE_WINDOW: std::time::Duration = std::time::Duration::from_secs(5 * 60);
