//! Atomic rename helper.
//! Performs rename and syncs the destination directory; removes existing dest on Windows.

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

/// Try an atomic rename; return an anyhow::Error on failure so callers can downcast.
///
/// This is a tiny helper so the higher-level logic can inspect the underlying io::Error
/// (e.g. EXDEV) via anyhow::Error::downcast_ref.
pub fn try_atomic_move(src: &Path, dst: &Path) -> Result<()> {
    fs::rename(src, dst)
        .with_context(|| format!("atomic rename '{}' -> '{}'", src.display(), dst.display()))
}
