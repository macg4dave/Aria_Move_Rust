//! Resolving the source path.
//! - If the caller provides a concrete path, use it if it exists.
//! - Otherwise, scan download_base (up to a shallow depth) and pick the most
//!   recently modified regular file within the configured recent_window.
//!
//! Notes:
//! - Uses a single-pass walk (no Vec allocation or re-statting) for efficiency.
//! - Re-validates the chosen path before returning to avoid TOCTOU surprises.

use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::config::types::Config;

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// Otherwise find the newest file under download_base modified within recent_window.
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    // 1) Prefer explicitly provided path when it exists.
    if let Some(p) = maybe_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        warn!("Provided source path does not exist: {}", p.display());
    }

    // 2) Compute recency cutoff and scan once, tracking the newest candidate.
    let cutoff = SystemTime::now()
        .checked_sub(config.recent_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut newest: Option<(SystemTime, PathBuf)> = None;

    for entry in WalkDir::new(&config.download_base)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        // Metadata fetch is done once; no re-stat later.
        if let Ok(meta) = entry.metadata() {
            if let Ok(modified) = meta.modified() {
                if modified >= cutoff {
                    if let Some((best_time, _)) = &newest {
                        if modified > *best_time {
                            newest = Some((modified, entry.into_path()));
                        }
                    } else {
                        newest = Some((modified, entry.into_path()));
                    }
                }
            }
        }
    }

    // 3) Return the newest candidate if still present.
    if let Some((_, path)) = newest {
        // Best-effort recheck that it still exists (avoid returning a stale path).
        if path.try_exists().unwrap_or(false) {
            info!("Auto-selected most recent: {}", path.display());
            return Ok(path);
        }
        bail!(
            "Most recent file '{}' disappeared after discovery",
            path.display()
        );
    }

    bail!(
        "No recently modified file found under {}",
        config.download_base.display()
    )
}
