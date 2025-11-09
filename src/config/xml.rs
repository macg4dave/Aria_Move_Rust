//! XML configuration support.
//! - Loads settings from config.xml (quick_xml).
//! - Creates a secure template if missing (unless ARIA_MOVE_CONFIG is set).
//! - Exposes helpers to ensure a default config exists.
//!
//! Notes:
//! - This module only reads/writes the config file; directory validation happens elsewhere.
//! - Unknown XML fields cause a hard failure (panic) to surface misconfigurations early.

use anyhow::{Context, Result};
use quick_xml::de::from_str as from_xml_str;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
// duration no longer parsed from XML; keep runtime default in Config
use tracing::{debug, info};

use super::paths::{default_config_path, default_log_path, path_has_symlink_ancestor};
use super::{COMPLETED_BASE_DEFAULT, DOWNLOAD_BASE_DEFAULT};

use crate::config::types::{Config, LogLevel};
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
    #[serde(rename = "preserve_permissions")]
    preserve_permissions: Option<bool>,
    #[serde(rename = "disable_locks")]
    disable_locks: Option<bool>,
}

// Reduce visual complexity of the return type used by load_config_from_xml().
type LoadedConfig = (
    PathBuf,          // download_base
    PathBuf,          // completed_base
    Option<LogLevel>, // log_level
    Option<PathBuf>,  // log_file
    bool,             // preserve_metadata
    bool,             // preserve_permissions
    bool,             // disable_locks
);

/// Read config from XML. OS-aware default path used if ARIA_MOVE_CONFIG not set.
/// Returns None if no meaningful settings are present or the file doesn’t exist.
pub fn load_config_from_xml() -> Option<LoadedConfig> {
    // 1) Choose config path:
    //    - ARIA_MOVE_CONFIG (if set)
    //    - default per-platform path (best-effort)
    // Resolve env override via default_config_path() to keep logic (rel/dir) consistent
    let env_set = env::var_os("ARIA_MOVE_CONFIG").is_some();
    let cfg_path = default_config_path().ok()?;

    // 2) If missing: create a template (only when using default path), then return None.
    if !cfg_path.exists() {
        if !env_set {
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
                panic!(
                    "Unknown field in aria_move config {}: {}. Refusing to start.",
                    cfg_path.display(),
                    msg
                );
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
    let download_base = parsed
        .download_base
        .as_deref()
        .map(|s| PathBuf::from(s.trim()));
    let completed_base = parsed
        .completed_base
        .as_deref()
        .map(|s| PathBuf::from(s.trim()));
    let log_level = parsed
        .log_level
        .as_deref()
        .and_then(|s| s.trim().parse::<LogLevel>().ok());
    let log_file = parsed.log_file.as_deref().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(PathBuf::from(trimmed))
        }
    });
    let preserve_metadata = parsed.preserve_metadata.unwrap_or(false);
    let preserve_permissions = parsed.preserve_permissions.unwrap_or(false);
    let disable_locks = parsed.disable_locks.unwrap_or(false);

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
        preserve_metadata,
        preserve_permissions,
        disable_locks,
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
        r#"<!--
    aria_move configuration (XML)

    Boolean flags (true/false):
        preserve_metadata      -> copy permissions + timestamps (+ xattrs when feature enabled)
        preserve_permissions   -> copy only permissions (mode on Unix, readonly on Windows)
        disable_locks          -> disable directory locking (for ZFS/NFS/network shares in containers)

    Other fields:
        download_base          -> directory where new/partial downloads appear
        completed_base         -> directory where completed items are moved
        log_level              -> quiet | normal | info | debug
        log_file               -> path to log file (optional; stdout/stderr still used)

    Notes:
        - CLI flags override XML values.
        - Setting preserve_metadata implies permissions; preserve_permissions is ignored if preserve_metadata=true.
        - Set disable_locks=true only if you encounter "Permission denied (os error 13)" on ZFS/NFS shares in containers.
-->
<config>
    <download_base>{}</download_base>
    <completed_base>{}</completed_base>
    <log_level>normal</log_level>
    <log_file>{}</log_file>
    <preserve_metadata>false</preserve_metadata>
    <preserve_permissions>false</preserve_permissions>
    <disable_locks>false</disable_locks>
</config>
"#,
        DOWNLOAD_BASE_DEFAULT, COMPLETED_BASE_DEFAULT, suggested_log
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

// Map XmlConfig -> Config (used by both loaders)
fn xml_to_config(parsed: XmlConfig) -> Config {
    let default_cfg = Config::default();
    let download_base = parsed
        .download_base
        .as_deref()
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from(DOWNLOAD_BASE_DEFAULT));
    let completed_base = parsed
        .completed_base
        .as_deref()
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|| PathBuf::from(COMPLETED_BASE_DEFAULT));
    let log_file = match parsed.log_file.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(PathBuf::from(s)),
        _ => default_cfg.log_file.clone(),
    };
    let log_level = parsed
        .log_level
        .as_deref()
        .and_then(|s| s.trim().parse::<LogLevel>().ok())
        .unwrap_or(default_cfg.log_level);
    let preserve_metadata = parsed.preserve_metadata.unwrap_or(false);
    let preserve_permissions = if preserve_metadata {
        false
    } else {
        parsed.preserve_permissions.unwrap_or(false)
    };
    let disable_locks = parsed.disable_locks.unwrap_or(false);
    Config {
        download_base,
        completed_base,
        log_level,
        log_file,
        dry_run: false,
        preserve_metadata,
        preserve_permissions,
        disable_locks,
    }
}

/// Load a Config from a specific XML file path (quick_xml).
pub fn load_config_from_xml_path(path: &Path) -> Result<Config> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("read config xml '{}'", path.display()))?;
    let parsed: XmlConfig = from_xml_str(&contents)
        .with_context(|| format!("parse config xml '{}'", path.display()))?;
    Ok(xml_to_config(parsed))
}

/// If ARIA_MOVE_CONFIG is set, load and return that Config; otherwise Ok(None).
pub fn load_config_from_xml_env() -> Result<Option<Config>> {
    if let Some(p) = env::var_os("ARIA_MOVE_CONFIG") {
        eprintln!("[DEBUG] ARIA_MOVE_CONFIG is set to: {:?}", p);
        let cfg = load_config_from_xml_path(Path::new(&p))?;
        eprintln!(
            "[DEBUG] Loaded config - download_base: {}",
            cfg.download_base.display()
        );
        eprintln!(
            "[DEBUG] Loaded config - completed_base: {}",
            cfg.completed_base.display()
        );
        return Ok(Some(cfg));
    }
    eprintln!("[DEBUG] ARIA_MOVE_CONFIG not set, will use default config path");
    Ok(None)
}

/// Try loading Config from the platform default config.xml path.
/// Returns Ok(Some(cfg)) if the file exists and parses; Ok(None) if missing.
pub fn load_config_from_default_xml() -> Result<Option<Config>> {
    let path = default_config_path().context("resolve default config path")?;
    if !path.exists() {
        return Ok(None);
    }
    eprintln!("[INFO] Using config from default XML: {}", path.display());
    let cfg = load_config_from_xml_path(&path)?;
    Ok(Some(cfg))
}
