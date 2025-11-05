//! XML configuration support.
//! Loads settings from config.xml, creates a secure template if needed, and ensures a default config exists.

use anyhow::Result;
use quick_xml::de::from_str as from_xml_str;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tracing::{debug, info};

#[cfg(unix)]
use libc;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt as UnixOpenOptionsExt;

use super::paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
use super::{DOWNLOAD_BASE_DEFAULT, COMPLETED_BASE_DEFAULT};

use crate::config::types::LogLevel;

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
    // optional override of recent_window in seconds
    #[serde(rename = "recent_window_seconds")]
    recent_window_seconds: Option<u64>,
}

// Reduce visual complexity of the return type used by load_config_from_xml().
type LoadedConfig = (
    PathBuf,
    PathBuf,
    Option<LogLevel>,
    Option<PathBuf>,
    Duration,
    bool,
);

/// Read config from XML. OS-aware default path used if ARIA_MOVE_CONFIG not set.
pub fn load_config_from_xml() -> Option<LoadedConfig> {
    let env_path = env::var("ARIA_MOVE_CONFIG").ok().map(PathBuf::from);
    let cfg_path = env_path.clone().or_else(default_config_path)?;

    if !cfg_path.exists() {
        if env_path.is_none() {
            let _ = create_template_config(&cfg_path);
        }
        return None;
    }

    let content = fs::read_to_string(&cfg_path).ok()?;

    let parsed: XmlConfig = match from_xml_str(&content) {
        Ok(x) => x,
        Err(e) => {
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

    let download_base = parsed.download_base.map(PathBuf::from);
    let completed_base = parsed.completed_base.map(PathBuf::from);
    let log_level = parsed.log_level.and_then(|s| LogLevel::parse(&s));
    let log_file = parsed.log_file.map(PathBuf::from);
    let recent_window = parsed
        .recent_window_seconds
        .map(Duration::from_secs)
        .unwrap_or(Duration::from_secs(60 * 5));
    let preserve_metadata = parsed.preserve_metadata.unwrap_or(false);

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
        log_file.or_else(default_log_path),
        recent_window,
        preserve_metadata,
    ))
}

/// Create default template config file and parent directory (best-effort permissions).
/// Uses O_NOFOLLOW + create_new on Unix to avoid following attacker-controlled symlinks.
pub fn create_template_config(path: &Path) -> Result<()> {
    if path_has_symlink_ancestor(path)? {
        return Err(anyhow::anyhow!(
            "Refusing to create config: ancestor of {} is a symlink",
            path.display()
        ));
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }

    let suggested_log = default_log_path()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/path/to/aria_move.log".into());

    let content = format!(
        "<config>\n  <download_base>{}</download_base>\n  <completed_base>{}</completed_base>\n  <log_level>normal</log_level>\n  <log_file>{}</log_file>\n  <!-- optional: preserve file permissions and mtime when moving (default: false) -->\n  <preserve_metadata>false</preserve_metadata>\n</config>\n",
        DOWNLOAD_BASE_DEFAULT,
        COMPLETED_BASE_DEFAULT,
        suggested_log
    );

    #[cfg(unix)]
    {
        use std::fs::OpenOptions;
        let mut opts = OpenOptions::new();
        opts.write(true)
            .create_new(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW);
        let mut f = opts.open(path)?;
        use std::io::Write;
        f.write_all(content.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        fs::write(path, content)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }

    info!("Created template config at {}", path.display());
    Ok(())
}

/// Create default config if ARIA_MOVE_CONFIG not set; return created path so CLI can inform the user.
pub fn ensure_default_config_exists() -> Option<PathBuf> {
    if env::var("ARIA_MOVE_CONFIG").is_ok() {
        return None;
    }
    let cfg_path = default_config_path()?;
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