//! Platform-specific helpers (macOS/Linux/Windows).

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(any(unix, target_os = "macos"))]
pub(crate) mod temp;

#[cfg(any(unix, target_os = "macos"))]
mod common_unix;

// Unix but not macOS (e.g., Linux)
#[cfg(all(unix, not(target_os = "macos")))]
mod unix;

#[cfg(windows)]
pub use windows::{
    check_disk_space, ensure_secure_directory, open_log_file_secure_append, set_dir_mode_0700,
    set_file_mode_0600, write_config_secure_new_0600,
};

#[cfg(target_os = "macos")]
pub use macos::{
    check_disk_space, open_log_file_secure_append, set_dir_mode_0700, set_file_mode_0600,
    write_config_secure_new_0600,
};

#[cfg(all(unix, not(target_os = "macos")))]
pub use unix::{
    check_disk_space, open_log_file_secure_append, set_dir_mode_0700, set_file_mode_0600,
    write_config_secure_new_0600,
};
