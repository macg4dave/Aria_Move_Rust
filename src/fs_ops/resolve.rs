//! Resolving the source path.
//! - If the caller provides a concrete path, use it if it exists.
//! - Otherwise, scan download_base (up to a shallow depth) and pick the most
//!   recently modified regular file within the configured recent_window.
//!
//! Notes:
//! - Uses a single-pass walk (no Vec allocation or re-statting) for efficiency.
//! - Re-validates the chosen path before returning to avoid TOCTOU surprises.

use anyhow::Result;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, Instant};
use tracing::{debug, info, warn, trace, instrument};
use walkdir::WalkDir;

use crate::config::types::Config;
use crate::errors::AriaMoveError;
use crate::shutdown;

const MAX_DEPTH: usize = 4;
const DENY_SUFFIXES: &[&str] = &[".part", ".aria2", ".tmp"];

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// Otherwise find the newest file under download_base modified within recent_window.
#[instrument(level = "debug", skip(config), fields(base=%config.download_base.display(), recent_secs = config.recent_window.as_secs()))]
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    // 1) Prefer explicitly provided path when it exists.
    if let Some(p) = maybe_path {
        if p.exists() {
            // Accept regular files, and symlinks that ultimately point to regular files.
            match std::fs::symlink_metadata(p) {
                Ok(meta) => {
                    let ft = meta.file_type();
                    if ft.is_file() {
                        return Ok(p.to_path_buf());
                    } else if ft.is_symlink() {
                        if let Ok(dm) = std::fs::metadata(p) {
                            if dm.is_file() {
                                return Ok(p.to_path_buf());
                            }
                        }
                        return Err(AriaMoveError::ProvidedNotFile(p.to_path_buf()).into());
                    } else {
                        return Err(AriaMoveError::ProvidedNotFile(p.to_path_buf()).into());
                    }
                }
                Err(_) => {
                    // Fall through to scanning with a warning.
                    warn!("Provided source path is not accessible: {}", p.display());
                }
            }
        } else {
            warn!("Provided source path does not exist: {}", p.display());
        }
    }

    // 2) Compute recency cutoff and scan once, tracking the newest candidate.
    let now = SystemTime::now();
    let strict_recent = config.recent_window > Duration::ZERO;
    let cutoff = if strict_recent {
        now.checked_sub(config.recent_window)
            .unwrap_or(SystemTime::UNIX_EPOCH)
    } else {
        SystemTime::UNIX_EPOCH
    };

    // Validate base exists and is a directory
    match std::fs::metadata(&config.download_base) {
        Ok(m) if m.is_dir() => {}
        _ => return Err(AriaMoveError::BaseInvalid(config.download_base.clone()).into()),
    }

    let started = Instant::now();
    let mut scanned = 0usize;
    let mut errors = 0usize;
    let mut denied = 0usize;
    let mut newest_recent: Option<(SystemTime, PathBuf)> = None;
    let mut newest_overall: Option<(SystemTime, PathBuf)> = None;

    let walker = WalkDir::new(&config.download_base)
        .follow_links(false)
        .min_depth(1)
        .max_depth(MAX_DEPTH);
    for item in walker.into_iter() {
        if shutdown::is_requested() {
            return Err(AriaMoveError::Interrupted.into());
        }
        match item {
            Ok(entry) => {
                if !entry.file_type().is_file() { continue; }
                let path = entry.path();
                let name = entry.file_name().to_string_lossy();
                if DENY_SUFFIXES.iter().any(|s| name.ends_with(s)) { continue; }

                scanned += 1;
                // Metadata once
                match entry.metadata() {
                    Ok(meta) => {
                        if meta.len() == 0 { /* optionally skip zero-length */ }
                        match meta.modified() {
                            Ok(modified) => {
                                // Track newest overall
                                update_newest(&mut newest_overall, modified, path.to_path_buf());
                                // Track newest within cutoff if strict_recent
                                if modified >= cutoff {
                                    update_newest(&mut newest_recent, modified, path.to_path_buf());
                                }
                            }
                            Err(_) => { errors += 1; }
                        }
                    }
                    Err(e) => {
                        errors += 1;
                            if let Some(ioe) = e.io_error() {
                                if ioe.kind() == std::io::ErrorKind::PermissionDenied { denied += 1; }
                            }
                    }
                }
            }
            Err(e) => {
                errors += 1;
                    if let Some(code) = e.io_error().and_then(|ioe| ioe.raw_os_error()) {
                        trace!(code, "walkdir error raw_os_error");
                    }
                    if let Some(ioe) = e.io_error() {
                        if let Some(code) = ioe.raw_os_error() { trace!(code, "walkdir error raw_os_error"); }
                        if ioe.kind() == std::io::ErrorKind::PermissionDenied { denied += 1; }
                    }
            }
        }
    }

    // 3) Return the newest candidate if still present.
    let chosen = newest_recent.or(newest_overall);
    if let Some((_, path)) = chosen {
        if path.try_exists().unwrap_or(false) {
            // Re-validate still a regular file
            if let Ok(m) = std::fs::metadata(&path) {
                if m.is_file() {
                    let elapsed = started.elapsed();
                    debug!(scanned, errors, denied, millis = elapsed.as_millis() as u64, "auto-selected most recent");
                    info!("Auto-selected most recent: {}", path.display());
                    return Ok(path);
                }
            }
        }
        return Err(AriaMoveError::Disappeared(path).into());
    }

    // Nothing at all found
    Err(AriaMoveError::NoneFound(config.download_base.clone()).into())
}

fn update_newest(slot: &mut Option<(SystemTime, PathBuf)>, t: SystemTime, path: PathBuf) {
    match slot {
        None => *slot = Some((t, path)),
        Some((best_t, best_p)) => {
            match t.cmp(best_t) {
                Ordering::Greater => *slot = Some((t, path)),
                Ordering::Equal => {
                    if path < *best_p { *slot = Some((t, path)); }
                }
                Ordering::Less => {}
            }
        }
    }
}
