//! Default path helpers and symlink checks.
//! - Determines OS-appropriate config/log paths (with `ARIA_MOVE_CONFIG` override for config).
//! - Keeps config and log file colocated (same directory) for easier discovery.
//! - Detects symlinked ancestors for safety (avoid writing logs under a symlinked parent).
//!
//! Notes:
//! - These functions only compute paths; they do not create directories/files. Callers must
//!   create parent directories as needed.
//! - Relative `ARIA_MOVE_CONFIG` values are resolved against the current working directory for
//!   clarity and to avoid surprises when launched from different shells.
//! - Fallback precedence (config):
//!     1. `ARIA_MOVE_CONFIG` env var (absolute or relative; relative resolved to CWD)
//!     2. `dirs::config_dir()` platform directory
//!     3. Platform-specific HOME fallback (Unix: `$HOME/.config/aria_move/config.xml`; Windows: `%USERPROFILE%/AppData/Roaming/aria_move/config.xml`)
//! - Fallback precedence (log):
//!     1. Parent directory of resolved config path (including env override)
//!     2. `dirs::data_dir()` platform directory (`.../aria_move/aria_move.log`)
//!     3. Platform-specific HOME fallback (Unix: `$HOME/.local/share/aria_move/aria_move.log`; Windows: `%USERPROFILE%/AppData/Local/aria_move/aria_move.log`)
//!
//! Potential future enhancements:
//! - Support XDG overrides (`XDG_CONFIG_HOME`, `XDG_DATA_HOME`).
//! - Distinguish when `ARIA_MOVE_CONFIG` points to a directory (append `config.xml`).

use anyhow::{anyhow, Context, Result};
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
        let candidate = PathBuf::from(&over);
        let resolved = if candidate.is_relative() {
            std::env::current_dir()
                .context("Failed to get current working directory while resolving ARIA_MOVE_CONFIG")?
                .join(candidate)
        } else {
            candidate
        };
        // If the override points to a directory, append config.xml
        if resolved.is_dir() || resolved.to_string_lossy().ends_with(std::path::MAIN_SEPARATOR) {
            return Ok(resolved.join("config.xml"));
        }
        return Ok(resolved);
    }

    if let Some(base) = config_dir() {
        return Ok(app_path(base, "config.xml"));
    }

    // HOME fallback (platform-specific). We attempt a Windows-specific layout if on Windows.
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow!("HOME/USERPROFILE not set for config fallback"))?;
    let home_path = PathBuf::from(home);
    if cfg!(windows) {
        // Use Roaming (similar to config_dir default) for config fallback.
        return Ok(home_path
            .join("AppData")
            .join("Roaming")
            .join("aria_move")
            .join("config.xml"));
    }
    Ok(home_path.join(".config").join("aria_move").join("config.xml"))
}

/// Return the default log file path as a PathBuf.
/// Uses the platform data dir (user-writable app data location).
/// If that is unavailable, falls back to $HOME/.local/share/aria_move/aria_move.log.
pub fn default_log_path() -> Result<PathBuf> {
    // 1) Colocate with config
    if let Ok(cfg_path) = default_config_path() {
        if let Some(parent) = cfg_path.parent() {
            return Ok(parent.join("aria_move.log"));
        }
    }

    // 2) data_dir fallback
    if let Some(base) = data_dir() {
        return Ok(app_path(base, "aria_move.log"));
    }

    // 3) HOME fallback (platform specific)
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow!("HOME/USERPROFILE not set for log fallback"))?;
    let home_path = PathBuf::from(home);
    if cfg!(windows) {
        return Ok(home_path
            .join("AppData")
            .join("Local")
            .join("aria_move")
            .join("aria_move.log"));
    }
    Ok(home_path
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
