//! Platform-specific helpers.
//! Hides OS differences (Unix/Windows) behind a uniform API so the rest of the code is platform-agnostic.
//!
//! Implementation detail:
//! - Select the concrete implementation module at compile time and alias it as `imp`.
//! - Re-export a stable set of functions from `imp` to avoid duplicating lists under cfg.

#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as imp;

#[cfg(not(unix))]
mod windows;
#[cfg(not(unix))]
use windows as imp;

// Stable public API surface (re-exported from the selected implementation).
pub use imp::{
    check_disk_space,
    ensure_secure_directory,
    open_log_file_secure_append,
    set_dir_mode_0700,
    set_file_mode_0600,
    write_config_secure_new_0600,
};
