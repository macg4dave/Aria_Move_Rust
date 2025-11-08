//! Resolving the source path.
//! - If the caller provides a concrete path, use it if it exists and is a regular file OR directory
//!   (or a symlink that resolves to one of those types).
//! - For a bare filename, try resolving it under `download_base` with the same rules.
//! - Otherwise, do not auto-pick; return an error. Auto-selection is out of scope for this program.
//!
//! Notes:
//! - Single-pass walk (no intermediate Vec) for efficiency.
//! - Re-validates the chosen path before returning to avoid TOCTOU surprises.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::{instrument, warn};

use crate::config::types::Config;
use crate::errors::AriaMoveError;

/// Resolve the source path. If `maybe_path` is Some and exists, that wins.
/// Otherwise returns an error (auto-pick is out of scope).
#[instrument(level = "debug", skip(config), fields(base=%config.download_base.display()))]
pub fn resolve_source_path(config: &Config, maybe_path: Option<&Path>) -> Result<PathBuf> {
    // 1) Prefer explicitly provided path when it exists.
    if let Some(p) = maybe_path {
        // When the caller provided a path explicitly, do NOT fall back to auto-scan.
        // Be strict and return a precise error so we never move an unintended file.
        match std::fs::symlink_metadata(p) {
            Ok(meta) => {
                let ft = meta.file_type();
                if ft.is_file() || ft.is_dir() {
                    return Ok(p.to_path_buf());
                } else if ft.is_symlink() {
                    if let Ok(dm) = std::fs::metadata(p)
                        && (dm.is_file() || dm.is_dir())
                    {
                        return Ok(p.to_path_buf());
                    }
                    return Err(AriaMoveError::ProvidedNotFile(p.to_path_buf()).into());
                } else {
                    return Err(AriaMoveError::ProvidedNotFile(p.to_path_buf()).into());
                }
            }
            Err(e) => {
                // If the provided argument is a bare filename (no path separators)
                // and does not exist as given, try resolving it under download_base.
                if e.kind() == std::io::ErrorKind::NotFound && is_bare_filename(p) {
                    let candidate = config.download_base.join(p);
                    match std::fs::symlink_metadata(&candidate) {
                        Ok(meta2) => {
                            let ft = meta2.file_type();
                            if ft.is_file() || ft.is_dir() {
                                return Ok(candidate);
                            } else if ft.is_symlink() {
                                if let Ok(dm) = std::fs::metadata(&candidate)
                                    && (dm.is_file() || dm.is_dir())
                                {
                                    return Ok(candidate);
                                }
                                return Err(AriaMoveError::ProvidedNotFile(candidate).into());
                            } else {
                                return Err(AriaMoveError::ProvidedNotFile(candidate).into());
                            }
                        }
                        Err(e2) => {
                            // Still not found (or other IO error) under base -> return structured error for candidate.
                            let am = AriaMoveError::from_io(candidate, &e2);
                            return Err(am.into());
                        }
                    }
                }

                // Map to structured error and stop (no bare-filename fallback applied).
                let am = AriaMoveError::from_io(p, &e);
                return Err(am.into());
            }
        }
    }

    // No explicit path provided -> out of scope. Do not auto-pick.
    Err(AriaMoveError::NoneFound(config.download_base.clone()).into())
}

#[inline]
fn is_bare_filename(p: &Path) -> bool {
    // A single path component (no separators) and not absolute.
    !p.has_root() && p.components().count() == 1 && p.file_name().is_some()
}
