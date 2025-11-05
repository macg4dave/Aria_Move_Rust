//! I/O helper utilities.
//! Converts io::Error values into action-oriented anyhow::Error messages with context and hints.

use anyhow::anyhow;
use std::io;
use std::path::Path;

#[cfg(unix)]
use libc;

/// Turn a low-level io::Error into an actionable message with context and hints.
pub fn io_error_with_help<'a>(
    op: &'a str,
    path: &'a Path,
) -> impl FnOnce(io::Error) -> anyhow::Error + 'a {
    move |e: io::Error| {
        let mut msg = format!("{} '{}': {}", op, path.display(), e);
        if let Some(code) = e.raw_os_error() {
            #[cfg(unix)]
            {
                match code {
                    libc::EACCES | libc::EPERM => {
                        msg.push_str(
                            " — permission denied; check ownership and write permissions.",
                        );
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
                    _ => {}
                }
            }
        } else {
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
                _ => {}
            }
        }
        anyhow!(msg)
    }
}
