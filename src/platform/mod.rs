//! Platform-specific helpers (macOS/Linux/Windows).

#[cfg(windows)]
mod windows;

// One Unix adapter shared by Linux and macOS.
#[cfg(unix)]
mod unix;

#[cfg(windows)]
pub use windows::{
    check_disk_space, ensure_secure_directory, open_log_file_secure_append, set_dir_mode_0700,
    set_file_mode_0600, write_config_secure_new_0600,
};

#[cfg(unix)]
pub use unix::{
    check_disk_space, open_log_file_secure_append, set_dir_mode_0700, set_file_mode_0600,
    write_config_secure_new_0600,
};

mod filesystem;
pub(crate) use filesystem::{open_directory, open_regular_read, rename_noreplace, sync_directory};
