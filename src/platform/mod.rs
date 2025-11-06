//! Platform-specific helpers (Unix/Windows).

#[cfg(windows)]
mod windows;
#[cfg(unix)]
mod unix;

#[cfg(windows)]
pub use windows::{
    check_disk_space, open_log_file_secure_append, set_dir_mode_0700, set_file_mode_0600,
    write_config_secure_new_0600,
};

#[cfg(unix)]
pub use unix::{
    check_disk_space, open_log_file_secure_append, set_dir_mode_0700, set_file_mode_0600,
    write_config_secure_new_0600,
};
