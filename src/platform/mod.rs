//! Platform-specific helpers.
//! This module hides OS differences (Unix/Windows) behind a uniform API so
//! the rest of the codebase can remain platform-agnostic.

#[cfg(unix)]
mod unix;
#[cfg(not(unix))]
mod windows;

#[cfg(unix)]
pub use unix::{
    check_disk_space, ensure_secure_directory, open_log_file_secure_append, set_dir_mode_0700,
    set_file_mode_0600, write_config_secure_new_0600,
};

#[cfg(not(unix))]
pub use windows::{
    check_disk_space, ensure_secure_directory, open_log_file_secure_append, set_dir_mode_0700,
    set_file_mode_0600, write_config_secure_new_0600,
};
