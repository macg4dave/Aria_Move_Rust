//! XML configuration support.
//! - Loads settings from config.xml (quick_xml).
//! - Creates a secure template if missing (unless ARIA_MOVE_CONFIG is set).
//! - Exposes helpers to ensure a default config exists.
//!
//! Notes:
//! - This module only reads/writes the config file; directory validation happens elsewhere.
//! - Unknown XML fields cause a hard failure (panic) to surface misconfigurations early.

use anyhow::Result;
use quick_xml::de::from_str as from_xml_str;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info};

use super::paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
use super::{COMPLETED_BASE_DEFAULT, DOWNLOAD_BASE_DEFAULT};

use crate::config::types::LogLevel;
use crate::platform::{set_dir_mode_0700, set_file_mode_0600, write_config_secure_new_0600};

/// Struct mirroring the XML config for deserialization.
#[derive(Debug, Deserialize)]
#[serde(rename = "config")]
#[serde(deny_unknown_fields)]
struct XmlConfig {
    #[serde(rename = "download_base")]
    download_base: Option<String>,
    #[serde(rename = "completed_base")]
    completed_base: Option<String>,
    #[serde(rename = "log_level")]
    log_level: Option<String>,
    #[serde(rename = "log_file")]
    log_file: Option<String>,
    #[serde(rename = "preserve_metadata")]
    preserve_metadata: Option<bool>,
    /// Optional override of recent_window in seconds
    #[serde(rename = "recent_window_seconds")]
    recent_window_seconds: Option<u64>,
}

// Reduce visual complexity of the return type used by load_config_from_xml().
type LoadedConfig = (
    PathBuf,          // download_base
    PathBuf,          // completed_base
    Option<LogLevel>, // log_level
    Option<PathBuf>,  // log_file
    Duration,         // recent_window
    bool,             // preserve_metadata
);

const DEFAULT_RECENT_SECS: u64 = 300;

/// Read config from XML. OS-aware default path used if ARIA_MOVE_CONFIG not set.
/// Returns None if no meaningful settings are present or the file doesn’t exist.
pub fn load_config_from_xml() -> Option<LoadedConfig> {
    // 1) Choose config path:
    //    - ARIA_MOVE_CONFIG (if set)
    //    - default per-platform path (best-effort)
    let env_path = env::var_os("ARIA_MOVE_CONFIG").map(PathBuf::from);
    let cfg_path = env_path
        .clone()
        .or_else(|| default_config_path().ok())?; // Option<PathBuf>

    // 2) If missing: create a template (only when using default path), then return None.
    if !cfg_path.exists() {
        if env_path.is_none() {
            let _ = create_template_config(&cfg_path);
        }
        return None;
    }

    // 3) Read and parse
    let content = fs::read_to_string(&cfg_path).ok()?;
    let parsed: XmlConfig = match from_xml_str(&content) {
        Ok(x) => x,
        Err(e) => {
            // Fail hard on unknown field (serde deny_unknown_fields); else, log and return None.
            let msg = e.to_string();
            if msg.contains("unknown field") {
                eprintln!(
                    "Unknown field in config {}: {}. Refusing to start.",
                    cfg_path.display(),
                    msg
                );
                panic!("Unknown field in aria_move config");
            }
            debug!(
                "Failed to parse config.xml at {}: {}",
                cfg_path.display(),
                msg
            );
            return None;
        }
    };

    // 4) Map fields
    let download_base = parsed.download_base.map(PathBuf::from);
    let completed_base = parsed.completed_base.map(PathBuf::from);
    let log_level = parsed.log_level.and_then(|s| LogLevel::parse(&s));
    let log_file = parsed.log_file.map(PathBuf::from);
    let recent_window = parsed
        .recent_window_seconds
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(DEFAULT_RECENT_SECS));
    let preserve_metadata = parsed.preserve_metadata.unwrap_or(false);

    // If no meaningful settings were provided, treat as "no config" so callers can use defaults.
    if download_base.is_none()
        && completed_base.is_none()
        && log_level.is_none()
        && log_file.is_none()
    {
        return None;
    }

    Some((
        download_base.unwrap_or_else(|| PathBuf::from(DOWNLOAD_BASE_DEFAULT)),
        completed_base.unwrap_or_else(|| PathBuf::from(COMPLETED_BASE_DEFAULT)),
        log_level,
        // best-effort default log path if not set
        log_file.or_else(|| default_log_path().ok()),
        recent_window,
        preserve_metadata,
    ))
}

/// Create default template config file and parent directory (best-effort permissions).
/// Uses secure creation to avoid following attacker-controlled symlinks on Unix.
pub fn create_template_config(path: &Path) -> Result<()> {
    if path_has_symlink_ancestor(path)? {
        return Err(anyhow::anyhow!(
            "Refusing to create config: ancestor of {} is a symlink",
            path.display()
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        let _ = set_dir_mode_0700(parent);
    }

    let suggested_log = default_log_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/path/to/aria_move.log".into());

    let content = format!(
        "<config>\n  <download_base>{}</download_base>\n  <completed_base>{}</completed_base>\n  <log_level>normal</log_level>\n  <log_file>{}</log_file>\n  <!-- optional: preserve file permissions and mtime when moving (default: false) -->\n  <preserve_metadata>false</preserve_metadata>\n  <!-- optional: override recent window in seconds (default: 300) -->\n  <recent_window_seconds>{}</recent_window_seconds>\n</config>\n",
        DOWNLOAD_BASE_DEFAULT,
        COMPLETED_BASE_DEFAULT,
        suggested_log,
        DEFAULT_RECENT_SECS
    );

    // Atomic, secure write (O_NOFOLLOW + create_new on Unix), then tighten perms.
    write_config_secure_new_0600(path, content.as_bytes())?;
    let _ = set_file_mode_0600(path);

    info!("Created template config at {}", path.display());
    Ok(())
}

/// Create default config if ARIA_MOVE_CONFIG not set; return created path so CLI can inform the user.
pub fn ensure_default_config_exists() -> Option<PathBuf> {
    if env::var_os("ARIA_MOVE_CONFIG").is_some() {
        return None;
    }

    let cfg_path = match default_config_path() {
        Ok(p) => p,
        Err(_) => return None,
    };

    if cfg_path.exists() {
        return None;
    }

    if let Ok(true) = path_has_symlink_ancestor(&cfg_path) {
        eprintln!(
            "Refusing to create template config because an existing ancestor is a symlink: {}",
            cfg_path.display()
        );
        return None;
    }

    match create_template_config(&cfg_path) {
        Ok(()) => Some(cfg_path),
        Err(e) => {
            eprintln!(
                "Failed to create template config at {}: {}",
                cfg_path.display(),
                e
            );
            None
        }
    }
}
