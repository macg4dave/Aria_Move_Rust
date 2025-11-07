//! Library crate root for aria_move.
//! Exposes a minimal, stable API and a convenient prelude for common imports.
//!
//! Notes:
//! - Re-exports come from concrete submodules to avoid accidental breakage if mod.rs changes.
//! - Prefer the `prelude` for downstream crates/tests to keep imports tidy.
//!
//! Example
//! -------
//!
//! ```no_run
//! use aria_move::prelude::*;
//! 
//! fn run() -> AMResult<()> {
//!     // Build a default config and wire minimal fields
//!     let mut cfg = Config::default();
//!     // cfg.download_base = "/incoming".into();
//!     // cfg.completed_base = "/completed".into();
//!     
//!     // Discover default config/log paths if needed
//!     let _default_cfg = default_config_path()?;
//! 
//!     // Resolve and move (illustrative; requires real paths)
//!     // let src = resolve_source_path(&cfg, None)?;
//!     // let _dest = move_entry(&cfg, &src)?;
//!     Ok(())
//! }
//! # let _ = AriaMoveError::Interrupted;
//! ```

pub mod config;
pub mod errors;
pub mod fs_ops;
pub mod platform;
pub mod shutdown;
pub mod utils;
pub mod output;
pub mod cli;

// Re-exports for tests and binaries
pub use config::types::{Config, LogLevel};

// Public API
pub use config::paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
pub use config::xml::{
    load_config_from_default_xml, load_config_from_xml_env, load_config_from_xml_path,
};

// Operations
pub use fs_ops::{move_dir, move_entry, move_file, resolve_source_path, safe_copy_and_rename};

// Errors
pub use errors::AriaMoveError;

/// Library-wide result alias using anyhow for ergonomic returns.
pub type AMResult<T> = anyhow::Result<T>;

/// Common imports for applications/tests using aria_move.
pub mod prelude {
    pub use crate::errors::AriaMoveError;
    pub use crate::config::types::{Config, LogLevel};
    pub use crate::fs_ops::{move_dir, move_entry, move_file, resolve_source_path, safe_copy_and_rename};
    pub use crate::config::paths::default_config_path;
    pub use crate::shutdown::request as request_shutdown;
    pub use crate::AMResult;
    pub use crate::errors::AriaMoveError as Error;
    pub use crate::errors::AriaMoveError as E;
    pub use crate::errors::AriaMoveError as AMError;
    pub use crate::errors::AriaMoveError as AriaError;
    pub use crate::errors::AriaMoveError as ErrorKind;
}
