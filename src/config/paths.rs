//! Default path helpers and symlink checks.
//! Determines OS-appropriate config/log paths and detects symlinked ancestors for safety.

use dirs::{config_dir, data_dir};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// OS-appropriate default config path.
pub fn default_config_path() -> Option<PathBuf> {
    if let Some(mut base) = config_dir() {
        base.push("aria_move");
        base.push("config.xml");
        Some(base)
    } else {
        std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join(".config")
                .join("aria_move")
                .join("config.xml")
        })
    }
}

/// OS-appropriate default log file path (data dir).
pub fn default_log_path() -> Option<PathBuf> {
    if let Some(mut base) = data_dir() {
        base.push("aria_move");
        // ensure dir exists (best-effort)
        let _ = fs::create_dir_all(&base);
        base.push("aria_move.log");
        Some(base)
    } else {
        std::env::var("HOME").ok().map(|h| {
            PathBuf::from(h)
                .join(".local")
                .join("share")
                .join("aria_move")
                .join("aria_move.log")
        })
    }
}

/// Return true if any existing ancestor of `path` is a symlink.
pub fn path_has_symlink_ancestor(path: &Path) -> io::Result<bool> {
    let mut p = path.parent();
    while let Some(anc) = p {
        if anc.exists() {
            let meta = fs::symlink_metadata(anc)?;
            if meta.file_type().is_symlink() {
                return Ok(true);
            }
        }
        p = anc.parent();
    }
    Ok(false)
}