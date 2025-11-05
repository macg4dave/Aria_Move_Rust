//! Advisory move lock.
//! Uses a sidecar lock file and fs2::FileExt to ensure only one process moves a path at a time.

use anyhow::{bail, Result};
use fs2::FileExt;
use std::path::{Path, PathBuf};

use super::helpers::io_error_with_help;

/// Guard that holds an exclusive lock on a sidecar lock file.
/// The lock is released when this value is dropped. We also try to remove the
/// lock file on drop (best-effort).
pub(super) struct MoveLock {
    _file: std::fs::File, // retained for RAII; keep field to hold lock
    path: PathBuf,
}

impl Drop for MoveLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path); // best-effort cleanup
    }
}

/// Acquire an exclusive advisory lock for the given source path.
pub(super) fn acquire_move_lock(src: &Path) -> Result<MoveLock> {
    let lock_path = {
        let mut p = src.to_path_buf();
        let ext = match p.extension() {
            Some(e) => format!("{}.aria_move.lock", e.to_string_lossy()),
            None => "aria_move.lock".to_string(),
        };
        p.set_extension(ext);
        p
    };

    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }

    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(io_error_with_help("open lock file", &lock_path))?;

    match file.try_lock_exclusive() {
        Ok(()) => Ok(MoveLock {
            _file: file,
            path: lock_path,
        }),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                bail!(
                    "Another aria_move process is operating on '{}'; try again later",
                    src.display()
                );
            }
            Err(anyhow::anyhow!(
                "failed to acquire lock for '{}': {} — is another process running?",
                src.display(),
                e
            ))
        }
    }
}
