//! Config and log file locations. Pure path computation; nothing is created here.
//!
//! Config precedence (the `--config` flag is handled by the binary first):
//! 1. `ARIA_MOVE_CONFIG` (a file, or a directory containing `config.xml`;
//!    relative values resolve against the current directory)
//! 2. `/etc/aria_move/config.xml` on Unix, when it exists
//! 3. the per-user config directory (`~/.config`, `~/Library/Application
//!    Support`, `%APPDATA%`)
//!
//! The default log lives beside the config file, unless the config is in a
//! system directory (`/etc`), in which case the per-user data directory is used.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub const CONFIG_ENV: &str = "ARIA_MOVE_CONFIG";
const CONFIG_NAME: &str = "config.xml";
const LOG_NAME: &str = "aria_move.log";
const SYSTEM_CONFIG: &str = "/etc/aria_move/config.xml";

/// The config file used when `--config` is not given.
pub fn default_config_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os(CONFIG_ENV) {
        return resolve_config_path(Path::new(&path));
    }
    if cfg!(unix) && Path::new(SYSTEM_CONFIG).exists() {
        return Ok(PathBuf::from(SYSTEM_CONFIG));
    }
    let base = dirs::config_dir().context("cannot determine the user configuration directory")?;
    Ok(base.join("aria_move").join(CONFIG_NAME))
}

/// Make an explicit config path absolute; a directory means its `config.xml`.
pub fn resolve_config_path(path: &Path) -> Result<PathBuf> {
    let absolute = std::path::absolute(path)
        .with_context(|| format!("resolve config path '{}'", path.display()))?;
    Ok(if absolute.is_dir() {
        absolute.join(CONFIG_NAME)
    } else {
        absolute
    })
}

/// The log file used when the config does not name one.
pub fn default_log_path(config_path: &Path) -> Option<PathBuf> {
    match config_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() && !is_system_dir(parent) => {
            Some(parent.join(LOG_NAME))
        }
        _ => dirs::data_dir().map(|dir| dir.join("aria_move").join(LOG_NAME)),
    }
}

fn is_system_dir(dir: &Path) -> bool {
    cfg!(unix) && dir.starts_with("/etc")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_is_colocated_with_a_user_config() {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("config.xml");
        assert_eq!(default_log_path(&config), Some(temp.path().join(LOG_NAME)));
    }

    #[cfg(unix)]
    #[test]
    fn system_config_logs_to_the_user_data_directory() {
        let log = default_log_path(Path::new(SYSTEM_CONFIG));
        assert_eq!(
            log,
            dirs::data_dir().map(|d| d.join("aria_move").join(LOG_NAME))
        );
        assert!(!log.unwrap().starts_with("/etc"));
    }

    #[test]
    fn directories_resolve_to_their_config_file() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(
            resolve_config_path(temp.path()).unwrap(),
            std::path::absolute(temp.path()).unwrap().join(CONFIG_NAME)
        );
        let file = temp.path().join("custom.xml");
        assert_eq!(resolve_config_path(&file).unwrap(), file);
        let relative = resolve_config_path(Path::new("relative.xml")).unwrap();
        assert!(relative.is_absolute() && relative.ends_with("relative.xml"));
    }
}
