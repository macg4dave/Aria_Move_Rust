//! I/O helper utilities.
//!
//! Provides small adapters to enrich io::Error with actionable context/hints,
//! usable with map_err in both io::Result and anyhow::Result code paths.
//!
//! Usage:
//!   // in functions returning anyhow::Result<_>
//!   fs::create_dir_all(dir).map_err(io_error_with_help("create dir", dir))?;
//!
//!   // in functions returning io::Result<_>
//!   File::open(p).map_err(io_error_with_help_io("open file", p))?;

use anyhow::anyhow;
use std::io;
use std::path::Path;

#[cfg(unix)]
use libc;

/// Format a human-friendly message with op/path plus platform-aware hints.
fn build_message(op: &str, path: &Path, e: &io::Error) -> String {
    let mut msg = format!("{} '{}': {}", op, path.display(), e);

    if let Some(code) = e.raw_os_error() {
        // Platform-specific hints by raw OS code.
        #[cfg(unix)]
        {
            match code {
                libc::EACCES | libc::EPERM => {
                    msg.push_str(" — permission denied; check ownership and write permissions.");
                }
                libc::EXDEV => {
                    msg.push_str(" — cross-filesystem; atomic rename not possible.");
                }
                libc::EBUSY => {
                    msg.push_str(" — resource busy; ensure no other process is writing.");
                }
                libc::ENOENT => {
                    msg.push_str(" — path not found; verify it exists.");
                }
                libc::EEXIST => {
                    msg.push_str(" — already exists; pick a unique name or remove the target.");
                }
                libc::ENOSPC => {
                    msg.push_str(" — insufficient space on device.");
                }
                libc::EROFS => {
                    msg.push_str(" — read-only filesystem; cannot write here.");
                }
                libc::ELOOP => {
                    msg.push_str(" — too many symbolic link levels (ELOOP); possible symlink cycle.");
                }
                libc::ENAMETOOLONG => {
                    msg.push_str(" — filename or path too long; shorten path segments.");
                }
                libc::EMFILE => {
                    msg.push_str(" — process file descriptor limit reached; close files or raise limits.");
                }
                libc::ENFILE => {
                    msg.push_str(" — system-wide file table overflow; reduce open files.");
                }
                _ => {}
            }
        }
        #[cfg(windows)]
        {
            // Common Win32 errors
            match code {
                5 => msg.push_str(" — access denied; check permissions."),          // ERROR_ACCESS_DENIED
                17 => msg.push_str(" — not same device; cross-filesystem move."),   // ERROR_NOT_SAME_DEVICE
                32 => msg.push_str(" — sharing violation; file is in use."),        // ERROR_SHARING_VIOLATION
                2 | 3 => msg.push_str(" — path not found; verify it exists."),      // FILE/ PATH NOT FOUND
                80 => msg.push_str(" — already exists; pick a unique name."),       // ERROR_FILE_EXISTS
                112 => msg.push_str(" — insufficient disk space."),                 // ERROR_DISK_FULL
                19 => msg.push_str(" — write protected / read-only media."),        // ERROR_WRITE_PROTECT
                206 => msg.push_str(" — filename or path too long (MAX_PATH exceeded)."), // ERROR_FILENAME_EXCED_RANGE
                4 => msg.push_str(" — too many open files; close handles or increase limit."), // ERROR_TOO_MANY_OPEN_FILES
                _ => {}
            }
        }
        // Include OS code for diagnostics
        msg.push_str(&format!(" [os code: {}]", code));
    } else {
        // Fallback to Kind-based hints
        match e.kind() {
            io::ErrorKind::PermissionDenied => {
                msg.push_str(" — permission denied; check ownership and write permissions.");
            }
            io::ErrorKind::NotFound => {
                msg.push_str(" — path not found; verify it exists.");
            }
            io::ErrorKind::AlreadyExists => {
                msg.push_str(" — already exists; remove or choose a unique name.");
            }
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => {
                msg.push_str(" — busy/timed out; retry after the current write finishes.");
            }
            _ => {}
        }
    }

    msg
}

/// Adapter for anyhow::Result code.
/// Returns a closure suitable for `.map_err(...)` that converts io::Error -> anyhow::Error.
pub fn io_error_with_help<'a>(
    op: &'a str,
    path: &'a Path,
) -> impl FnOnce(io::Error) -> anyhow::Error + 'a {
    move |e: io::Error| anyhow!(build_message(op, path, &e))
}

/// Adapter for io::Result code (when the surrounding function returns io::Result).
/// Returns a closure suitable for `.map_err(...)` that converts io::Error -> io::Error
/// with enriched context in the message while preserving the original ErrorKind.
pub fn io_error_with_help_io<'a>(
    op: &'a str,
    path: &'a Path,
) -> impl FnOnce(io::Error) -> io::Error + 'a {
    move |e: io::Error| io::Error::new(e.kind(), build_message(op, path, &e))
}
