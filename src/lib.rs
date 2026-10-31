//! aria_move: move completed aria2 downloads from a download base into a
//! completed base, by exclusive rename or a verified copy across filesystems.
//!
//! ```no_run
//! use aria_move::{Config, config, move_entry, resolve_source_path};
//!
//! fn run() -> anyhow::Result<()> {
//!     let mut cfg = Config::new("/data/incoming", "/data/completed");
//!     config::validate_and_normalize(&mut cfg)?;
//!     let src = resolve_source_path(&cfg, Some("Some Download".as_ref()))?;
//!     let dest = move_entry(&cfg, &src)?;
//!     println!("moved to {}", dest.display());
//!     Ok(())
//! }
//! ```

pub mod cli;
pub mod config;
pub mod errors;
pub mod fs_ops;
pub mod logging;
pub mod output;
mod platform;
pub mod shutdown;

pub use config::{Config, LogLevel};
pub use errors::AriaMoveError;
pub use fs_ops::{move_entry, resolve_source_path};

/// Library-wide result alias.
pub type AMResult<T> = anyhow::Result<T>;
