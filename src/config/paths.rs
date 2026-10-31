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
use anyhow::{Context, Result, anyhow};
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
        return resolve_config_path(Path::new(&over));
    }

    // Unix system-wide config: prefer /etc/aria_move/config.xml if it exists.
    // This allows systemd services or root-managed installs to provide a global config.
    // We only pick it when present; we do not attempt to create it by default.
    if cfg!(unix) {
        let etc_path = PathBuf::from("/etc/aria_move/config.xml");
        if etc_path.exists() {
            return Ok(etc_path);
        }
    }

    if let Some(base) = config_dir() {
        return Ok(app_path(base, "config.xml"));
    }

    // HOME fallback (platform-specific). We attempt a Windows-specific layout if on Windows.
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
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
    Ok(home_path
        .join(".config")
        .join("aria_move")
        .join("config.xml"))
}

/// Return the default log file path as a PathBuf.
/// Uses the platform data dir (user-writable app data location).
/// If that is unavailable, falls back to $HOME/.local/share/aria_move/aria_move.log.
pub fn default_log_path() -> Result<PathBuf> {
    // 1) Colocate with config, unless the parent is a system directory like /etc (not writable).
    if let Ok(cfg_path) = default_config_path()
        && let Some(parent) = cfg_path.parent()
        && !(cfg!(unix) && parent.starts_with("/etc"))
    {
        return Ok(parent.join("aria_move.log"));
    }

    // 2) data_dir fallback
    if let Some(base) = data_dir() {
        return Ok(app_path(base, "aria_move.log"));
    }

    // 3) HOME fallback (platform specific)
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
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

/// Return true if an existing ancestor is a symlink or Windows reparse point.
/// Metadata errors other than missing components are propagated.
pub fn path_has_symlink_ancestor(path: &Path) -> io::Result<bool> {
    for ancestor in path.ancestors().skip(1) {
        if ancestor.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(ancestor) {
            Ok(meta) if is_link(&meta) => return Ok(true),
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(false)
}

pub(crate) fn is_link(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Include junctions and all reparse-point types, not only symbolic links.
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

pub(crate) fn reject_symlinks(path: &Path) -> io::Result<()> {
    let leaf_is_link = match fs::symlink_metadata(path) {
        Ok(meta) => is_link(&meta),
        Err(e) if e.kind() == io::ErrorKind::NotFound => false,
        Err(e) => return Err(e),
    };
    if leaf_is_link || path_has_symlink_ancestor(path)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Refusing symlink or reparse point: {}", path.display()),
        ));
    }
    Ok(())
}

/// Resolve a config override without changing process environment.
pub fn resolve_config_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path).context("resolve config override")?;
    if absolute.is_dir() {
        Ok(absolute.join("config.xml"))
    } else {
        Ok(absolute)
    }
}

/// Normalize a prospective path using its nearest existing ancestor, without writes.
pub(crate) fn normalize_missing(path: &Path) -> Result<PathBuf> {
    use std::path::Component;
    let absolute = std::path::absolute(path)?;
    let mut normalized = PathBuf::new();
    for part in absolute.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(part),
        }
    }
    reject_symlinks(&normalized)?;
    let mut existing = normalized.as_path();
    let mut missing = Vec::new();
    while !existing.try_exists()? {
        missing.push(
            existing
                .file_name()
                .context("path has no existing ancestor")?,
        );
        existing = existing.parent().context("path has no parent")?;
    }
    let mut result = dunce::canonicalize(existing)?;
    for component in missing.into_iter().rev() {
        result.push(component);
    }
    Ok(result)
}

/// Create only missing directories. Existing directory permissions are never changed.
pub(crate) fn create_directory(path: &Path) -> io::Result<()> {
    reject_symlinks(path)?;
    if path.is_dir() {
        return Ok(());
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        create_directory(parent)?;
    }
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    #[cfg(windows)]
    let _ = &mut builder;
    match builder.create(path) {
        Ok(()) => {}
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists && path.is_dir() => {}
        Err(e) => return Err(e),
    }
    reject_symlinks(path)
}
