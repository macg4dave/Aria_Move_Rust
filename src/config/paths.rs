//! Default path helpers and symlink checks.
//! - Determines OS-appropriate config/log paths (with ARIA_MOVE_CONFIG override for config).
//! - Detects symlinked ancestors for safety.
//!
//! NOTE: These functions only compute paths; they do not create directories/files.
//!       Callers are responsible for creating parents if desired.

use anyhow::{anyhow, Result};
use dirs::{config_dir, data_dir};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Build "<base>/aria_move/<filename>".
fn app_path(mut base: PathBuf, filename: &str) -> PathBuf {
    base.push("aria_move");
    base.push(filename);
    base
}

/// Return the default config file path as a PathBuf.
/// Precedence:
/// 1) ARIA_MOVE_CONFIG environment variable (absolute or relative)
/// 2) Platform config dir (e.g., macOS: ~/Library/Application Support, Linux: ~/.config, Windows: %APPDATA%)
/// 3) HOME fallback (Linux-style ~/.config)
pub fn default_config_path() -> Result<PathBuf> {
    if let Some(over) = std::env::var_os("ARIA_MOVE_CONFIG") {
        return Ok(PathBuf::from(over));
    }

    if let Some(base) = config_dir() {
        return Ok(app_path(base, "config.xml"));
    }

    // Fallback to $HOME/.config/aria_move/config.xml
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME not set"))?;
    Ok(PathBuf::from(home).join(".config").join("aria_move").join("config.xml"))
}

/// Return the default log file path as a PathBuf.
/// Uses the platform data dir (user-writable app data location).
/// If that is unavailable, falls back to $HOME/.local/share/aria_move/aria_move.log.
pub fn default_log_path() -> Result<PathBuf> {
    // Prefer the same directory as the config file so config and logs live together.
    // This respects ARIA_MOVE_CONFIG if set.
    if let Ok(cfg_path) = default_config_path() {
        if let Some(parent) = cfg_path.parent() {
            return Ok(parent.join("aria_move.log"));
        }
    }

    // Fallback to data_dir (legacy behavior)
    if let Some(base) = data_dir() {
        return Ok(app_path(base, "aria_move.log"));
    }

    // Final fallback to $HOME/.local/share/aria_move/aria_move.log
    let home = std::env::var_os("HOME").ok_or_else(|| anyhow!("HOME not set"))?;
    Ok(PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("aria_move")
        .join("aria_move.log"))
}

/// Return true if any existing ancestor of `path` is a symlink.
/// Non-existent ancestors are skipped safely.
pub fn path_has_symlink_ancestor(path: &Path) -> io::Result<bool> {
    let mut cur = path.parent();
    while let Some(dir) = cur {
        if dir.exists() {
            let meta = fs::symlink_metadata(dir)?;
            if meta.file_type().is_symlink() {
                return Ok(true);
            }
        }
        cur = dir.parent();
    }
    Ok(false)
}
