use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::config::Config;

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// Otherwise find the newest file under download_base modified within recent_window.
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = maybe_path {
        if p.exists() {
            return Ok(p.to_path_buf());
        }
        warn!("Provided source path does not exist: {}", p.display());
    }

    let cutoff = SystemTime::now()
        .checked_sub(config.recent_window)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut candidates: Vec<_> = WalkDir::new(&config.download_base)
        .min_depth(1)
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok().map(|t| (t, e.into_path())))
        })
        .filter(|(modified, _)| *modified >= cutoff)
        .map(|(_, p)| p)
        .collect();

    candidates.sort_by_key(|p| {
        std::cmp::Reverse(std::fs::metadata(p).ok().and_then(|m| m.modified().ok()))
    });

    if let Some(path) = candidates.first() {
        if !path.exists() {
            bail!(
                "Most recent file '{}' disappeared after discovery",
                path.display()
            );
        }
        info!("Auto-selected most recent: {}", path.display());
        return Ok(path.to_path_buf());
    }
    bail!(
        "No recently modified file found under {}",
        config.download_base.display()
    )
}
