//! Configuration facade. XML owns loading; validate owns preparation policy.
pub mod paths;
pub mod types;
mod validate;
pub mod xml;

pub use paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
pub use types::{Config, LogLevel};
pub use validate::validate_and_normalize;
pub use xml::{
    LoadResult, ensure_selected_config_exists as ensure_default_config_exists, load_or_init,
};

/// Default download base; installations should supply explicit disjoint directories.
pub const DOWNLOAD_BASE_DEFAULT: &str = "/mnt/World";
/// Historical completed base default. Unedited nested defaults fail validation.
pub const COMPLETED_BASE_DEFAULT: &str = "/mnt/World/completed";
